//! One retained benchmark network with durable, ordered continuation acknowledgements.
//!
//! The adapter owns kernel process identities and worker exit. This owner never
//! reports per-attempt process quiescence and never treats completion as acceptance.

use super::*;
use rustix::fs::{AtFlags, Mode, OFlags, open, openat, statat};
use std::os::{
    fd::FromRawFd,
    unix::fs::{FileExt, FileTypeExt, MetadataExt},
};

const PROTOCOL: &str = "AtomicPrivateSettlementV1";
const IDENTITY: &[&str] = &[
    "version",
    "protocol",
    "scope_sha256",
    "campaign_id",
    "plan_sha256",
    "session_id",
    "session_invocation_nonce",
    "session_request_sha256",
];
const ATTEMPT: &[&str] = &[
    "attempt_id",
    "request_id",
    "invocation_nonce",
    "session_attempt_index",
];
const STOP_REASONS: &[&str] = &[
    "attempt_failed",
    "attempt_timed_out",
    "attempt_incomplete",
    "validation_failed",
    "publication_failed",
    "setup_failed",
    "transport_interrupted",
    "cleanup_failed",
];
const TIMING_KINDS: &[&str] = &[
    "measurement_ready",
    "measurement_begin",
    "measurement_finished",
    "measurement_recorded",
];

#[derive(Clone, Debug, PartialEq, Eq, norito::JsonSerialize, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct RecordRef {
    path: String,
    sha256: String,
    bytes: u64,
}

fn text<'a>(value: &'a HarnessJsonValue, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(HarnessJsonValue::as_str)
        .ok_or_else(|| eyre!("missing textual session field: {key}"))
}

fn number(value: &HarnessJsonValue, key: &str) -> Result<u64> {
    value
        .get(key)
        .and_then(HarnessJsonValue::as_u64)
        .ok_or_else(|| eyre!("missing unsigned session field: {key}"))
}

fn field<'a>(value: &'a HarnessJsonValue, key: &str) -> Result<&'a HarnessJsonValue> {
    value
        .get(key)
        .ok_or_else(|| eyre!("missing session field: {key}"))
}

fn exact(value: &HarnessJsonValue, names: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("session record is not an object"))?;
    ensure!(
        object.len() == names.len() && names.iter().all(|name| object.contains_key(*name)),
        "unexpected session record fields"
    );
    Ok(())
}

fn digest(raw: &[u8]) -> String {
    hex::encode(Sha256::digest(raw))
}

fn checked_digest(value: &str) -> Result<()> {
    ensure!(
        lowercase_digest(value, &[64]),
        "invalid session digest or nonce"
    );
    Ok(())
}

fn canonical(value: &HarnessJsonValue) -> Result<Vec<u8>> {
    fn inspect(value: &HarnessJsonValue, depth: usize) -> Result<()> {
        ensure!(depth <= 32, "control JSON nesting exceeds bound");
        match value {
            HarnessJsonValue::Null | HarnessJsonValue::Bool(_) | HarnessJsonValue::String(_) => {}
            HarnessJsonValue::Number(_) => {
                ensure!(value.as_u64().is_some(), "control number is not u64");
            }
            HarnessJsonValue::Array(items) => {
                for item in items {
                    inspect(item, depth + 1)?;
                }
            }
            HarnessJsonValue::Object(items) => {
                for item in items.values() {
                    inspect(item, depth + 1)?;
                }
            }
        }
        Ok(())
    }
    inspect(value, 0)?;
    let raw = canonical_harness_json_bytes(value)?;
    ensure!(
        !raw.is_empty() && raw.len() <= HARNESS_MAX_JSON_BYTES,
        "control JSON exceeds byte bound"
    );
    Ok(raw)
}

fn decode(raw: &[u8]) -> Result<HarnessJsonValue> {
    ensure!(
        !raw.is_empty() && raw.len() <= HARNESS_MAX_JSON_BYTES,
        "control JSON exceeds byte bound"
    );
    let value: HarnessJsonValue = norito::json::from_slice(raw)?;
    ensure!(
        value.as_object().is_some() && canonical(&value)? == raw,
        "control JSON is not exact canonical object bytes"
    );
    Ok(value)
}

fn subset(value: &HarnessJsonValue, names: &[&str]) -> Result<HarnessJsonValue> {
    let mut out = BTreeMap::new();
    for name in names {
        out.insert((*name).to_owned(), field(value, name)?.clone());
    }
    Ok(HarnessJsonValue::Object(out))
}

fn join(left: &HarnessJsonValue, right: &HarnessJsonValue, names: &[&str]) -> Result<()> {
    ensure!(
        canonical(&subset(left, names)?)? == canonical(&subset(right, names)?)?,
        "session or attempt identity differs"
    );
    Ok(())
}

fn relative(path: &str) -> Result<Vec<&str>> {
    ensure!(
        !path.is_empty()
            && path.len() <= 4096
            && !path.starts_with('/')
            && !path.contains('\\')
            && !path.chars().any(|c| c.is_control()),
        "unsafe session relative path"
    );
    let parts = path.split('/').collect::<Vec<_>>();
    ensure!(
        parts
            .iter()
            .all(|part| !part.is_empty() && *part != "." && *part != ".."),
        "noncanonical session path"
    );
    Ok(parts)
}

fn record_ref(value: &HarnessJsonValue) -> Result<RecordRef> {
    exact(value, &["path", "sha256", "bytes"])?;
    let row: RecordRef = norito::json::from_value(value.clone())?;
    relative(&row.path)?;
    checked_digest(&row.sha256)?;
    ensure!(
        row.bytes > 0 && row.bytes <= (HARNESS_MAX_JSON_BYTES + 4) as u64,
        "record length exceeds bound"
    );
    Ok(row)
}

fn metadata(info: &fs::Metadata) -> (u64, u64, u32, u64, u64, i64, i64, i64, i64) {
    (
        info.dev(),
        info.ino(),
        info.mode(),
        info.nlink(),
        info.len(),
        info.mtime(),
        info.mtime_nsec(),
        info.ctime(),
        info.ctime_nsec(),
    )
}

fn owner_only(info: &fs::Metadata, directory: bool) -> Result<()> {
    ensure!(
        info.uid() == rustix::process::geteuid().as_raw()
            && info.mode() & 0o077 == 0
            && if directory {
                info.is_dir()
            } else {
                info.is_file() && info.nlink() == 1
            },
        "record is not an owner-only ordinary object"
    );
    Ok(())
}

struct RecordRoot {
    path: PathBuf,
    directory: File,
    identity: (u64, u64),
}

impl RecordRoot {
    fn open(path: &Path) -> Result<Self> {
        ensure!(
            path.is_absolute() && path.canonicalize()? == path,
            "session root must be canonical"
        );
        // Walk every component relative to a held directory, rejecting ancestor links.
        let mut dir = File::from(open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        for part in path.components().skip(1) {
            dir = File::from(openat(
                &dir,
                part.as_os_str(),
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )?);
        }
        let info = dir.metadata()?;
        owner_only(&info, true)?;
        let root = Self {
            path: (path.to_owned()),
            directory: dir,
            identity: (info.dev(), info.ino()),
        };
        root.validate()?;
        Ok(root)
    }

    fn validate(&self) -> Result<()> {
        let held = self.directory.metadata()?;
        let named = fs::symlink_metadata(&self.path)?;
        owner_only(&held, true)?;
        ensure!(
            named.is_dir()
                && (named.dev(), named.ino()) == self.identity
                && (held.dev(), held.ino()) == self.identity
                && self.path.canonicalize()? == self.path,
            "session record root was substituted"
        );
        Ok(())
    }

    fn parents(&self, path: &str) -> Result<(Vec<File>, Vec<String>)> {
        self.validate()?;
        let parts = relative(path)?;
        let mut parents = vec![self.directory.try_clone()?];
        let mut names = Vec::new();
        for part in &parts[..parts.len() - 1] {
            let next = File::from(openat(
                parents.last().unwrap(),
                *part,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
            )?);
            owner_only(&next.metadata()?, true)?;
            parents.push(next);
            names.push((*part).to_owned());
        }
        Ok((parents, names))
    }

    fn check_parents(&self, parents: &[File], names: &[String]) -> Result<()> {
        self.validate()?;
        for (index, name) in names.iter().enumerate() {
            let held = parents[index + 1].metadata()?;
            let named = statat(&parents[index], name.as_str(), AtFlags::SYMLINK_NOFOLLOW)?;
            owner_only(&held, true)?;
            ensure!(
                rustix::fs::FileType::from_raw_mode(named.st_mode)
                    == rustix::fs::FileType::Directory
                    && named.st_dev as u64 == held.dev()
                    && named.st_ino as u64 == held.ino(),
                "record parent was substituted"
            );
        }
        Ok(())
    }

    fn read(&self, reference: &RecordRef) -> Result<Vec<u8>> {
        record_ref(&norito::json::to_value(reference)?)?;
        let (parents, names) = self.parents(&reference.path)?;
        let before_parents = parents
            .iter()
            .map(|p| p.metadata().map(|m| metadata(&m)))
            .collect::<std::io::Result<Vec<_>>>()?;
        let leaf = relative(&reference.path)?.last().unwrap().to_string();
        let fd = File::from(openat(
            parents.last().unwrap(),
            leaf.as_str(),
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let before = fd.metadata()?;
        owner_only(&before, false)?;
        ensure!(before.len() == reference.bytes, "record length differs");
        let mut raw = vec![0; usize::try_from(reference.bytes)?];
        fd.read_exact_at(&mut raw, 0)?;
        let after = fd.metadata()?;
        let named = File::from(openat(
            parents.last().unwrap(),
            leaf.as_str(),
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        ensure!(
            metadata(&before) == metadata(&after)
                && metadata(&after) == metadata(&named.metadata()?)
                && digest(&raw) == reference.sha256,
            "record changed during authenticated read"
        );
        self.check_parents(&parents, &names)?;
        ensure!(
            parents
                .iter()
                .map(|p| p.metadata().map(|m| metadata(&m)))
                .collect::<std::io::Result<Vec<_>>>()?
                == before_parents,
            "record parent metadata changed during read"
        );
        Ok(raw)
    }

    fn publish(&self, path: &str, raw: &[u8]) -> Result<RecordRef> {
        ensure!(
            !raw.is_empty() && raw.len() <= HARNESS_MAX_JSON_BYTES + 4,
            "published record exceeds bound"
        );
        let (parents, names) = self.parents(path)?;
        let leaf = relative(path)?.last().unwrap().to_string();
        let mut fd = File::from(openat(
            parents.last().unwrap(),
            leaf.as_str(),
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_bits_truncate(0o600),
        )?);
        fd.write_all(raw)?;
        fd.sync_all()?;
        parents.last().unwrap().sync_all()?;
        let before = fd.metadata()?;
        owner_only(&before, false)?;
        let mut retained = vec![0; raw.len()];
        fd.read_exact_at(&mut retained, 0)?;
        let named = File::from(openat(
            parents.last().unwrap(),
            leaf.as_str(),
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        ensure!(
            before.len() == raw.len() as u64
                && retained == raw
                && metadata(&before) == metadata(&fd.metadata()?)
                && metadata(&before) == metadata(&named.metadata()?),
            "published record was substituted or changed"
        );
        self.check_parents(&parents, &names)?;
        Ok(RecordRef {
            path: (path.to_owned()),
            sha256: digest(raw),
            bytes: raw.len() as u64,
        })
    }

    fn located(&self, path: &str, sha256: &str) -> Result<RecordRef> {
        checked_digest(sha256)?;
        let (parents, names) = self.parents(path)?;
        let parts = relative(path)?;
        let stat = statat(
            parents.last().unwrap(),
            *parts.last().unwrap(),
            AtFlags::SYMLINK_NOFOLLOW,
        )?;
        ensure!(
            rustix::fs::FileType::from_raw_mode(stat.st_mode) == rustix::fs::FileType::RegularFile
                && stat.st_size > 0
                && stat.st_size <= (HARNESS_MAX_JSON_BYTES + 4) as _,
            "invalid bound record file"
        );
        self.check_parents(&parents, &names)?;
        Ok(RecordRef {
            path: (path.to_owned()),
            sha256: (sha256.to_owned()),
            bytes: stat.st_size as u64,
        })
    }
}

#[derive(Debug, norito::JsonDeserialize, norito::JsonSerialize)]
#[norito(deny_unknown_fields)]
struct PlannedAttempt {
    attempt_id: String,
    request_id: String,
    invocation_nonce: String,
    session_attempt_index: u64,
    request: RecordRef,
    output_directory: String,
}

#[derive(Debug, norito::JsonDeserialize, norito::JsonSerialize)]
#[norito(deny_unknown_fields)]
struct SessionRequest {
    version: u8,
    protocol: String,
    kind: String,
    scope_sha256: String,
    campaign_id: String,
    plan_sha256: String,
    session_id: String,
    session_invocation_nonce: String,
    workload_manifest_sha256: String,
    workload_manifest: HarnessJsonValue,
    commit: String,
    configuration_sha256: String,
    profile: String,
    participants: usize,
    seed: u64,
    warmups: u64,
    attempts: Vec<PlannedAttempt>,
}

fn policy(participants: usize) -> HarnessJsonValue {
    norito::json!({"version":1,"protocol":PROTOCOL,"kind":"matched_benchmark_payment_policy",
        "participants":participants,"primary_amount_base":42,"primary_amount_step":1,
        "sponsor_reimbursement_amount":5,"private_change_amount":7,"reserve_note_amount":1,
        "sponsor":"genesis_alice","derivation_domain":"iroha:matched-benchmark-workload:v1",
        "attempt_coordinates":["participants","seed","session_attempt_index","warmup"],
        "prefunding_policy":"session_union_of_disjoint_attempts"})
}

fn session_id(request: &SessionRequest) -> Result<String> {
    Ok(digest(&canonical(&norito::json!({
        "domain":"iroha:private-settlement:benchmark-session-plan:v1",
        "profile":(request.profile.clone()),"participants":(request.participants),"seed":(request.seed),
        "configuration_sha256":(request.configuration_sha256.clone()),
        "workload_manifest_sha256":(request.workload_manifest_sha256.clone()),
        "warmup_attempts":(request.warmups),
        "measured_attempts":((request.attempts.len() as u64).checked_sub(request.warmups).ok_or_else(|| eyre!("session has no measured allocation"))?),
    }))?))
}

fn validate_session(request: &SessionRequest) -> Result<()> {
    ensure!(
        request.version == 1
            && request.protocol == PROTOCOL
            && request.kind == "benchmark_session"
            && [2, 3, 4, 8, 16].contains(&request.participants)
            && matches!(request.profile.as_str(), "private" | "transparent_control")
            && (5..=1000).contains(&request.warmups)
            && request.attempts.len() as u64 > request.warmups
            && request.attempts.len() <= 1_000_000
            && lowercase_digest(&request.commit, &[40, 64]),
        "invalid retained session request"
    );
    let campaign = request.campaign_id.as_bytes();
    ensure!(
        !campaign.is_empty()
            && campaign.len() <= 64
            && (campaign[0].is_ascii_lowercase() || campaign[0].is_ascii_digit())
            && campaign
                .iter()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_' || *b == b'-'),
        "invalid campaign slug"
    );
    for value in [
        &request.scope_sha256,
        &request.plan_sha256,
        &request.session_id,
        &request.session_invocation_nonce,
        &request.workload_manifest_sha256,
        &request.configuration_sha256,
    ] {
        checked_digest(value)?;
    }
    ensure!(
        canonical(&request.workload_manifest)? == canonical(&policy(request.participants))?
            && digest(&canonical(&request.workload_manifest)?) == request.workload_manifest_sha256
            && session_id(request)? == request.session_id,
        "session policy or complete plan identity differs"
    );
    let mut ids = BTreeSet::new();
    let mut nonces = BTreeSet::new();
    let mut outputs = BTreeSet::new();
    for (index, attempt) in request.attempts.iter().enumerate() {
        checked_digest(&attempt.request_id)?;
        checked_digest(&attempt.invocation_nonce)?;
        record_ref(&norito::json::to_value(&attempt.request)?)?;
        relative(&attempt.output_directory)?;
        ensure!(
            attempt.session_attempt_index == index as u64
                && ids.insert(&attempt.request_id)
                && nonces.insert(&attempt.invocation_nonce)
                && outputs.insert(&attempt.output_directory),
            "session attempt inventory is reordered or duplicated"
        );
        let job = norito::json!({"kind":"benchmark","profile":(request.profile.clone()),"participants":(request.participants),"seed":(request.seed),
            "session_id":(request.session_id.clone()),"session_attempt_index":index,
            "warmup":(index < request.warmups as usize),"configuration_sha256":(request.configuration_sha256.clone()),"workload_manifest_sha256":(request.workload_manifest_sha256.clone())});
        ensure!(
            digest(&canonical(&job)?) == attempt.request_id,
            "request ID does not bind exact session job"
        );
        let expected = digest(&canonical(
            &norito::json!({"domain":"iroha:private-settlement:registered-attempt:v1",
            "scope_sha256":(request.scope_sha256.clone()),"campaign_id":(request.campaign_id.clone()),
            "plan_sha256":(request.plan_sha256.clone()),"request_id":(attempt.request_id.clone())}),
        )?);
        ensure!(
            expected == attempt.attempt_id,
            "attempt ID does not bind registered scope"
        );
        let leaf = attempt
            .output_directory
            .strip_prefix("attempts/")
            .ok_or_else(|| eyre!("attempt output is outside canonical inventory"))?;
        ensure!(
            leaf.len() == 5 + 1 + 64
                && leaf.as_bytes()[..5].iter().all(u8::is_ascii_digit)
                && leaf.as_bytes()[5] == b'-'
                && &leaf[6..] == attempt.request_id
                && attempt.request.path == format!("{}/request.json", attempt.output_directory),
            "attempt output/request locator differs"
        );
    }
    Ok(())
}

fn validate_message(
    value: &HarnessJsonValue,
    identity: &HarnessJsonValue,
    direction: &str,
) -> Result<()> {
    let mut names = IDENTITY.to_vec();
    names.extend([
        "channel",
        "direction",
        "sequence",
        "previous_message_sha256",
        "kind",
        "payload",
        "forwarded_from",
    ]);
    exact(value, &names)?;
    join(value, identity, IDENTITY)?;
    ensure!(
        text(value, "channel")? == "adapter_worker" && text(value, "direction")? == direction,
        "control channel or direction differs"
    );
    number(value, "sequence")?;
    checked_digest(text(value, "previous_message_sha256")?)?;
    let kind = text(value, "kind")?;
    let mut fields = if matches!(kind, "dispatch" | "accept" | "attempt_completed")
        || TIMING_KINDS.contains(&kind)
    {
        ATTEMPT.to_vec()
    } else {
        Vec::new()
    };
    fields.extend(match (direction, kind) {
        ("owner_to_child", "dispatch") => vec!["attempt_started", "request"],
        ("owner_to_child", "accept") => vec![
            "rust_terminal",
            "adapter_outcome",
            "response",
            "validation",
            "sample",
        ],
        ("owner_to_child", "stop") => vec!["active_attempt_id", "reason", "validation"],
        ("child_to_owner", "ready") => vec!["ready"],
        ("child_to_owner", "attempt_completed") => vec!["rust_terminal"],
        ("child_to_owner", "session_completed") => vec!["worker_terminal"],
        ("child_to_owner", "measurement_ready" | "measurement_finished") => vec!["marker"],
        ("owner_to_child", "measurement_begin") => vec!["marker", "process_observation"],
        ("owner_to_child", "measurement_recorded") => vec!["marker", "measurement_window"],
        _ => return Err(eyre!("control kind is invalid for direction")),
    });
    let payload = field(value, "payload")?;
    exact(payload, &fields)?;
    if fields.contains(&"attempt_id") {
        for key in &ATTEMPT[..3] {
            checked_digest(text(payload, key)?)?;
        }
        number(payload, "session_attempt_index")?;
    }
    for key in fields.iter().filter(|key| !ATTEMPT.contains(key)) {
        if kind == "stop" {
            match *key {
                "reason" => ensure!(
                    STOP_REASONS.contains(&text(payload, key)?),
                    "unknown typed stop reason"
                ),
                "active_attempt_id" => {
                    if field(payload, key)? != &HarnessJsonValue::Null {
                        checked_digest(text(payload, key)?)?;
                    }
                }
                _ => {
                    if field(payload, key)? != &HarnessJsonValue::Null {
                        record_ref(field(payload, key)?)?;
                    }
                }
            }
        } else {
            record_ref(field(payload, key)?)?;
        }
    }
    if direction == "owner_to_child" && !TIMING_KINDS.contains(&kind) {
        record_ref(field(value, "forwarded_from")?)?;
    } else {
        ensure!(
            field(value, "forwarded_from")? == &HarnessJsonValue::Null,
            "worker message cannot claim forwarding"
        );
    }
    Ok(())
}

fn decode_frame(raw: &[u8]) -> Result<HarnessJsonValue> {
    ensure!(
        raw.len() > 4
            && raw.len() <= HARNESS_MAX_JSON_BYTES + 4
            && u32::from_be_bytes(raw[..4].try_into()?) as usize == raw.len() - 4,
        "retained frame length differs"
    );
    decode(&raw[4..])
}

struct Chain {
    identity: HarnessJsonValue,
    direction: &'static str,
    sequence: u64,
    previous: String,
    poisoned: bool,
    prefix: String,
}

impl Chain {
    fn new(
        identity: HarnessJsonValue,
        started_sha256: &str,
        direction: &'static str,
    ) -> Result<Self> {
        checked_digest(started_sha256)?;
        let previous = digest(&canonical(
            &norito::json!({"domain":"iroha:private-settlement:session-control-chain:v1",
            "session_started_sha256":started_sha256,"channel":"adapter_worker","direction":direction}),
        )?);
        let prefix = format!("sessions/{}/control", text(&identity, "session_id")?);
        Ok(Self {
            identity,
            direction,
            sequence: 0,
            previous,
            poisoned: false,
            prefix,
        })
    }

    fn path(&self, suffix: &str) -> String {
        format!(
            "{}/worker.adapter_worker.{}.{:020}.{suffix}",
            self.prefix, self.direction, self.sequence
        )
    }

    fn advance(&mut self, value: &HarnessJsonValue, raw: &[u8]) -> Result<()> {
        ensure!(!self.poisoned, "control chain is poisoned");
        validate_message(value, &self.identity, self.direction)?;
        ensure!(
            number(value, "sequence")? == self.sequence
                && text(value, "previous_message_sha256")? == self.previous,
            "control chain is replayed, reordered or substituted"
        );
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| eyre!("control sequence overflow"))?;
        self.previous = digest(raw);
        Ok(())
    }

    fn receive(&mut self, root: &RecordRoot, reader: &mut impl Read) -> Result<HarnessJsonValue> {
        let result = (|| {
            ensure!(!self.poisoned, "control chain is poisoned");
            let mut raw = Vec::new();
            let mut wanted = 4;
            loop {
                let mut part = vec![0; (wanted - raw.len()).min(65536)];
                let n = match reader.read(&mut part) {
                    Ok(n) => n,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(error) => {
                        if !raw.is_empty() {
                            root.publish(&self.path("incomplete"), &raw)?;
                        }
                        return Err(error.into());
                    }
                };
                if n == 0 {
                    if !raw.is_empty() {
                        root.publish(&self.path("incomplete"), &raw)?;
                    }
                    return Err(eyre!("control pipe ended before a complete frame"));
                }
                raw.extend_from_slice(&part[..n]);
                if raw.len() == 4 && wanted == 4 {
                    let size = u32::from_be_bytes(raw[..4].try_into()?) as usize;
                    if size == 0 || size > HARNESS_MAX_JSON_BYTES {
                        root.publish(&self.path("incomplete"), &raw)?;
                        return Err(eyre!("declared control length exceeds bound"));
                    }
                    wanted = 4 + size;
                }
                if raw.len() == wanted {
                    break;
                }
            }
            root.publish(&self.path("frame"), &raw)?;
            let value = decode_frame(&raw)?;
            self.advance(&value, &raw)?;
            if TIMING_KINDS.contains(&text(&value, "kind")?) {
                return Ok(value);
            }
            let upstream_ref = record_ref(field(&value, "forwarded_from")?)?;
            let upstream = decode_frame(&root.read(&upstream_ref)?)?;
            let mut upstream_names = IDENTITY.to_vec();
            upstream_names.extend([
                "channel",
                "direction",
                "sequence",
                "previous_message_sha256",
                "kind",
                "payload",
                "forwarded_from",
            ]);
            exact(&upstream, &upstream_names)?;
            join(&upstream, &value, IDENTITY)?;
            ensure!(
                text(&upstream, "channel")? == "runner_adapter"
                    && text(&upstream, "direction")? == "owner_to_child"
                    && field(&upstream, "forwarded_from")? == &HarnessJsonValue::Null
                    && field(&upstream, "kind")? == field(&value, "kind")?
                    && canonical(field(&upstream, "payload")?)?
                        == canonical(field(&value, "payload")?)?,
                "adapter changed or invented its upstream owner message"
            );
            checked_digest(text(&upstream, "previous_message_sha256")?)?;
            number(&upstream, "sequence")?;
            Ok(value)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn send(
        &mut self,
        root: &RecordRoot,
        writer: &mut impl Write,
        kind: &str,
        payload: HarnessJsonValue,
    ) -> Result<RecordRef> {
        let result = (|| {
            ensure!(!self.poisoned, "control chain is poisoned");
            let mut value = self.identity.clone();
            let object = value.as_object_mut().unwrap();
            object.insert("channel".to_owned(), "adapter_worker".into());
            object.insert("direction".to_owned(), self.direction.into());
            object.insert("sequence".to_owned(), self.sequence.into());
            object.insert(
                "previous_message_sha256".to_owned(),
                self.previous.clone().into(),
            );
            object.insert("kind".to_owned(), kind.into());
            object.insert("payload".to_owned(), payload);
            object.insert("forwarded_from".to_owned(), HarnessJsonValue::Null);
            validate_message(&value, &self.identity, self.direction)?;
            let body = canonical(&value)?;
            let mut raw = u32::try_from(body.len())?.to_be_bytes().to_vec();
            raw.extend(body);
            let binding = root.publish(&self.path("frame"), &raw)?;
            writer.write_all(&raw)?;
            writer.flush()?;
            self.advance(&value, &raw)?;
            Ok(binding)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
}

#[allow(unsafe_code)]
fn take_control_pipes(read_fd: i32, write_fd: i32) -> Result<(File, File)> {
    ensure!(
        read_fd > 2 && write_fd > 2 && read_fd != write_fd,
        "invalid inherited control descriptors"
    );
    // The launcher transfers exclusive ownership of these exact two inherited
    // descriptors. No descriptor is fabricated, duplicated, or reused here.
    let reader = unsafe { File::from_raw_fd(read_fd) };
    let writer = unsafe { File::from_raw_fd(write_fd) };
    ensure!(
        reader.metadata()?.file_type().is_fifo() && writer.metadata()?.file_type().is_fifo(),
        "control descriptors must be dedicated pipes"
    );
    for fd in [&reader, &writer] {
        let flags = rustix::io::fcntl_getfd(fd)?;
        rustix::io::fcntl_setfd(fd, flags | rustix::io::FdFlags::CLOEXEC)?;
    }
    ensure!(
        (rustix::fs::fcntl_getfl(&reader)? & OFlags::RWMODE) == OFlags::RDONLY
            && (rustix::fs::fcntl_getfl(&writer)? & OFlags::RWMODE) == OFlags::WRONLY,
        "control pipe directions differ"
    );
    Ok((reader, writer))
}

pub(super) struct RetainedBenchmarkNetwork {
    pub(super) network: sandbox::SerializedNetwork,
    runtime: tokio::runtime::Runtime,
    coordinator: CoordinatorProcessV1,
    shape: TopologyShape,
    activated_height: u64,
    initial_inventory: Vec<RealProcessInventoryRowV1>,
}

impl RetainedBenchmarkNetwork {
    fn start(request: &SessionRequest, workloads: &[MatchedBenchmarkWorkloadV1]) -> Result<Self> {
        let shape = TopologyShape::new(request.participants);
        shape.validate()?;
        let seed = digest(&canonical(
            &norito::json!({"domain":"iroha:matched-benchmark-session-network:v1",
            "participants":(request.participants),"seed":(request.seed),
            "configuration_sha256":(request.configuration_sha256.clone()),
            "workload_manifest_sha256":(request.workload_manifest_sha256.clone()),
            "economic_vector_sha256":(workloads.iter().map(MatchedBenchmarkWorkloadV1::digest).collect::<Result<Vec<_>>>()?)}),
        )?);
        let builder = matched_benchmark_builder(shape, workloads, &seed)?;
        let context = format!(
            "atomic_private_settlement_retained_session_{}",
            request.session_id
        );
        let started = sandbox::start_network_blocking_or_skip(builder, &context)?;
        let (network, runtime) = sandbox::enforce_network_start_requirement(started, &context)?
            .ok_or_else(|| eyre!("retained release network was skipped"))?;
        verify_controller_readiness(&network, &runtime)?;
        let coordinator = CoordinatorProcessV1::start(&network.client())?;
        let activated_height = require_genesis_private_note_active(&network.client())?;
        let initial_inventory =
            collect_process_inventory(&network, &runtime, shape, &request.commit, &coordinator)?;
        Ok(Self {
            network,
            runtime,
            coordinator,
            shape,
            activated_height,
            initial_inventory,
        })
    }

    pub(super) fn inventory(&self, commit: &str) -> Result<Vec<RealProcessInventoryRowV1>> {
        let current = collect_process_inventory(
            &self.network,
            &self.runtime,
            self.shape,
            commit,
            &self.coordinator,
        )?;
        ensure!(
            canonical_harness_json_bytes(&current)?
                == canonical_harness_json_bytes(&self.initial_inventory)?,
            "retained network process inventory changed"
        );
        Ok(current)
    }

    pub(super) fn current_height(&self) -> Result<u64> {
        let height = self
            .network
            .client()
            .client()
            .get_privacy_capabilities()?
            .committed_height;
        ensure!(
            height >= self.activated_height,
            "retained session authority height moved backwards"
        );
        Ok(height)
    }

    fn shutdown(&mut self) -> Result<(bool, bool)> {
        self.runtime.block_on(self.network.shutdown());
        let mut absent = true;
        for peer in self.network.all_peers() {
            absent &= self.runtime.block_on(peer.process_id()).is_none();
        }
        ensure!(absent, "network owner still retains a live peer process");
        write_owner_only_atomic(
            &self.coordinator.root.path().join(COORDINATOR_SHUTDOWN_FILE),
            b"shutdown\n",
        )?;
        let started = Instant::now();
        loop {
            if let Some(status) = self.coordinator.child.try_wait()? {
                ensure!(
                    status.success(),
                    "coordinator exited unsuccessfully during shutdown"
                );
                return Ok((true, true));
            }
            if started.elapsed() >= FINALITY_TIMEOUT {
                return Err(benchmark_deadline_error(
                    BenchmarkDeadlineStageV1::CoordinatorAck,
                    FINALITY_TIMEOUT,
                    started.elapsed(),
                )
                .wrap_err("coordinator cleanup acknowledgement deadline"));
            }
            thread::sleep(POLL_INTERVAL);
        }
    }
}

// The assigned network endpoints are not a substitute for adapter-owned kernel socket observations.
fn validate_network_ports_document(
    document: &HarnessJsonValue,
    identity: &HarnessJsonValue,
    network_id: iroha::data_model::NetworkId,
    shape: TopologyShape,
    inventory: &[RealProcessInventoryRowV1],
) -> Result<()> {
    shape.validate()?;
    let mut fields = IDENTITY.to_vec();
    fields.extend(["kind", "network_id", "participants", "groups", "peers"]);
    exact(document, &fields)?;
    join(document, identity, IDENTITY)?;
    ensure!(
        text(document, "kind")? == "benchmark_network_ports"
            && number(document, "participants")? == u64::try_from(shape.participants)?
            && field(document, "network_id")? == &norito::json::to_value(&network_id)?,
        "session endpoint network identity differs"
    );
    let peers = field(document, "peers")?
        .as_array()
        .ok_or_else(|| eyre!("session endpoint rows are not an array"))?;
    ensure!(
        inventory.len() == shape.process_count() + 1
            && peers.len() == shape.process_count()
            && inventory[0].role == "coordinator"
            && inventory
                .iter()
                .all(|row| row.pid > 1 && row.health_observed)
            && inventory
                .iter()
                .map(|row| row.pid)
                .collect::<BTreeSet<_>>()
                .len()
                == inventory.len(),
        "session endpoint process inventory is incomplete or duplicated"
    );
    let mut torii_ports = Vec::new();
    let mut public_p2p_ports = Vec::new();
    let mut restricted_p2p_ports = Vec::new();
    for (index, (peer, process)) in peers.iter().zip(&inventory[1..]).enumerate() {
        exact(
            peer,
            &[
                "peer_index",
                "pid",
                "role",
                "dataspace_ordinal",
                "validator_ordinal",
                "torii",
                "p2p",
            ],
        )?;
        let lane = index / VALIDATORS_PER_LANE;
        let role = if lane == 0 {
            "global_validator"
        } else {
            "dataspace_validator"
        };
        let dataspace = (lane != 0).then(|| u64::try_from(lane - 1).expect("bounded ordinal"));
        let validator = Some(u64::try_from(index % VALIDATORS_PER_LANE)?);
        ensure!(
            number(peer, "peer_index")? == u64::try_from(index)?
                && number(peer, "pid")? == u64::from(process.pid)
                && text(peer, "role")? == role
                && process.role == role
                && field(peer, "dataspace_ordinal")? == &norito::json::to_value(&dataspace)?
                && process.dataspace_ordinal == dataspace
                && field(peer, "validator_ordinal")? == &norito::json::to_value(&validator)?
                && process.validator_ordinal == validator,
            "session endpoint does not join its exact validator process"
        );
        let visibility =
            if lane == 0 || shape.participant_visibility(lane - 1) == LaneVisibility::Public {
                "public"
            } else {
                "restricted"
            };
        for (kind, ports) in [
            ("torii", &mut torii_ports),
            (
                "p2p",
                if visibility == "public" {
                    &mut public_p2p_ports
                } else {
                    &mut restricted_p2p_ports
                },
            ),
        ] {
            let endpoint = field(peer, kind)?;
            if kind == "torii" {
                exact(endpoint, &["address", "port", "transport"])?;
            } else {
                exact(endpoint, &["address", "port", "transport", "visibility"])?;
                ensure!(
                    text(endpoint, "visibility")? == visibility,
                    "endpoint visibility differs from its committee"
                );
            }
            let port = u16::try_from(number(endpoint, "port")?)?;
            ensure!(
                port != 0
                    && text(endpoint, "address")? == "127.0.0.1"
                    && text(endpoint, "transport")? == "tcp",
                "endpoint is not the exact loopback TCP listener"
            );
            ports.push(port);
        }
    }
    for ports in [
        &mut torii_ports,
        &mut public_p2p_ports,
        &mut restricted_p2p_ports,
    ] {
        ports.sort_unstable();
    }
    let all = torii_ports
        .iter()
        .chain(&public_p2p_ports)
        .chain(&restricted_p2p_ports)
        .copied()
        .collect::<BTreeSet<_>>();
    ensure!(
        all.len() == 2 * shape.process_count(),
        "session endpoint ports overlap"
    );
    let expected = LeakagePortManifestV1 {
        version: 1,
        torii_ports,
        public_p2p_ports,
        restricted_p2p_ports,
    };
    ensure!(
        canonical(field(document, "groups")?)? == canonical(&norito::json::to_value(&expected)?)?,
        "session grouped endpoint inventory differs from exact peer rows"
    );
    Ok(())
}

impl RetainedBenchmarkNetwork {
    fn network_ports_document(
        &self,
        identity: &HarnessJsonValue,
        commit: &str,
    ) -> Result<HarnessJsonValue> {
        let inventory = self.inventory(commit)?;
        let mut peers = Vec::new();
        for (index, peer) in self.network.all_peers().enumerate() {
            let process = &inventory[index + 1];
            ensure!(
                self.runtime.block_on(peer.process_id()) == Some(process.pid),
                "endpoint peer PID changed"
            );
            let torii = peer.api_address();
            let p2p = peer.p2p_address();
            ensure!(
                torii.to_literal() == format!("127.0.0.1:{}", torii.port())
                    && p2p.to_literal() == format!("127.0.0.1:{}", p2p.port()),
                "non-loopback session endpoint"
            );
            let lane = index / VALIDATORS_PER_LANE;
            let visibility = if lane == 0
                || self.shape.participant_visibility(lane - 1) == LaneVisibility::Public
            {
                "public"
            } else {
                "restricted"
            };
            peers.push(norito::json!({"peer_index":index,"pid":(process.pid),"role":(process.role.clone()),
                "dataspace_ordinal":(process.dataspace_ordinal),"validator_ordinal":(process.validator_ordinal),
                "torii":{"address":"127.0.0.1","port":(torii.port()),"transport":"tcp"},
                "p2p":{"address":"127.0.0.1","port":(p2p.port()),"transport":"tcp","visibility":visibility}}));
        }
        let mut document = identity.clone();
        let row = document.as_object_mut().unwrap();
        row.insert("kind".to_owned(), "benchmark_network_ports".into());
        row.insert(
            "network_id".to_owned(),
            norito::json::to_value(&self.network.network_id())?,
        );
        row.insert("participants".to_owned(), self.shape.participants.into());
        row.insert(
            "groups".to_owned(),
            norito::json::to_value(&collect_network_port_manifest(&self.network, self.shape)?)?,
        );
        row.insert("peers".to_owned(), HarnessJsonValue::Array(peers));
        validate_network_ports_document(
            &document,
            identity,
            self.network.network_id(),
            self.shape,
            &inventory,
        )?;
        self.inventory(commit)?;
        Ok(document)
    }
}

fn attempt_identity(attempt: &PlannedAttempt) -> HarnessJsonValue {
    norito::json!({"attempt_id":(attempt.attempt_id.clone()),"request_id":(attempt.request_id.clone()),
        "invocation_nonce":(attempt.invocation_nonce.clone()),"session_attempt_index":(attempt.session_attempt_index)})
}

fn validate_attempt_request(
    session: &SessionRequest,
    attempt: &PlannedAttempt,
    raw: &[u8],
) -> Result<RealProcessBenchmarkRequestV1> {
    let value = decode(raw)?;
    let request: RealProcessBenchmarkRequestV1 = norito::json::from_value(value)?;
    validate_real_process_request(&request)?;
    ensure!(
        request.request_id == attempt.request_id
            && request.invocation_nonce == attempt.invocation_nonce
            && request.session_id == session.session_id
            && request.session_invocation_nonce == session.session_invocation_nonce
            && request.workload_manifest_sha256 == session.workload_manifest_sha256
            && request.session_attempt_index == attempt.session_attempt_index
            && request.commit == session.commit
            && request.configuration_sha256 == session.configuration_sha256
            && request.payload.profile == session.profile
            && request.participants == session.participants
            && request.seed == session.seed
            && request.payload.warmup == (attempt.session_attempt_index < session.warmups)
            && harness_json_object_u64(&request.configuration, "benchmark", "warmups_per_session")
                == Some(session.warmups),
        "prebound attempt request differs from complete session"
    );
    Ok(request)
}

fn validate_dispatch(
    root: &RecordRoot,
    message: &HarnessJsonValue,
    identity: &HarnessJsonValue,
    attempt: &PlannedAttempt,
    request_raw: &[u8],
    session_started: &RecordRef,
    previous_accept: Option<&RecordRef>,
) -> Result<()> {
    ensure!(
        text(message, "kind")? == "dispatch",
        "session expects dispatch after accepted predecessor"
    );
    let payload = field(message, "payload")?;
    join(payload, &attempt_identity(attempt), ATTEMPT)?;
    ensure!(
        record_ref(field(payload, "request")?)? == attempt.request
            && root.read(&attempt.request)? == request_raw,
        "dispatch substituted prebound request bytes"
    );
    let start_ref = record_ref(field(payload, "attempt_started")?)?;
    ensure!(
        start_ref.path == format!("{}/started.json", attempt.output_directory),
        "attempt start locator differs"
    );
    let start = decode(&root.read(&start_ref)?)?;
    let mut names = IDENTITY.to_vec();
    names.extend(ATTEMPT.iter().copied());
    names.extend([
        "ordinal",
        "session_started",
        "request",
        "outer_timeout_ms",
        "started_ns",
        "preceding_acceptance",
    ]);
    exact(&start, &names)?;
    join(&start, identity, IDENTITY)?;
    join(&start, payload, ATTEMPT)?;
    ensure!(
        record_ref(field(&start, "session_started")?)? == *session_started
            && record_ref(field(&start, "request")?)? == attempt.request
            && number(&start, "outer_timeout_ms")? > 0
            && number(&start, "started_ns")? > 0
            && attempt.output_directory
                == format!(
                    "attempts/{:05}-{}",
                    number(&start, "ordinal")?,
                    attempt.request_id
                ),
        "durable start differs from exact invocation"
    );
    match previous_accept {
        None => ensure!(
            attempt.session_attempt_index == 0
                && field(&start, "preceding_acceptance")? == &HarnessJsonValue::Null,
            "first start invented predecessor acceptance"
        ),
        Some(previous) => ensure!(
            attempt.session_attempt_index > 0
                && record_ref(field(&start, "preceding_acceptance")?)? == *previous,
            "successor start does not bind exact previously consumed owner acceptance"
        ),
    }
    Ok(())
}

fn validate_accept(
    root: &RecordRoot,
    message: &HarnessJsonValue,
    attempt: &PlannedAttempt,
    terminal_ref: &RecordRef,
    identity: &HarnessJsonValue,
) -> Result<RecordRef> {
    ensure!(
        text(message, "kind")? == "accept",
        "completed attempt lacks durable acceptance"
    );
    let payload = field(message, "payload")?;
    join(payload, &attempt_identity(attempt), ATTEMPT)?;
    ensure!(
        record_ref(field(payload, "rust_terminal")?)? == *terminal_ref,
        "acceptance substituted completed Rust terminal"
    );
    let mut retained = Vec::new();
    for key in [
        "rust_terminal",
        "adapter_outcome",
        "response",
        "validation",
        "sample",
    ] {
        let reference = record_ref(field(payload, key)?)?;
        let raw = root.read(&reference)?;
        let value: HarnessJsonValue = norito::json::from_slice(&raw)?;
        retained.push((key, reference, raw, value));
    }
    let terminal: RealProcessBenchmarkTerminalV1 = norito::json::from_slice(&retained[0].2)?;
    let RealProcessBenchmarkOutcomeV1::Succeeded(result) = &terminal.outcome else {
        return Err(eyre!(
            "acceptance cannot convert a failed or timed-out Rust terminal"
        ));
    };
    ensure!(
        terminal.request_id == attempt.request_id
            && terminal.invocation_nonce == attempt.invocation_nonce
            && terminal.request_sha256 == attempt.request.sha256,
        "acceptance cannot convert a failed, timed-out or substituted Rust terminal"
    );
    // The runner owns full semantic validation. Its durable acceptance must bind
    // its exact successful validation/sample and the adapter's successful output.
    // No statement here treats that acknowledgement as a worker-process exit.
    let validation = &retained[3].3;
    ensure!(
        text(validation, "request_id")? == attempt.request_id
            && text(validation, "attempt_id")? == attempt.attempt_id
            && field(validation, "passed")? == &HarnessJsonValue::Bool(true)
            && text(validation, "validation_kind")? == "accepted",
        "runner validation did not accept this exact attempt"
    );
    join(
        validation,
        identity,
        &["scope_sha256", "campaign_id", "plan_sha256"],
    )?;
    ensure!(
        record_ref(field(validation, "response")?)? == retained[2].1
            && record_ref(field(validation, "sample")?)? == retained[4].1,
        "accepted validation substituted its response or sample binding"
    );
    let adapter = &retained[1].3;
    ensure!(
        text(adapter, "status")? == "succeeded"
            && text(adapter, "request_id")? == attempt.request_id
            && text(adapter, "invocation_nonce")? == attempt.invocation_nonce
            && text(adapter, "request_sha256")? == attempt.request.sha256
            && record_ref(field(adapter, "rust_terminal")?)? == *terminal_ref,
        "acceptance lacks the exact successful adapter validation"
    );
    let response = &retained[2].3;
    ensure!(
        text(response, "request_id")? == attempt.request_id
            && text(response, "invocation_nonce")? == attempt.invocation_nonce
            && text(response, "request_sha256")? == attempt.request.sha256,
        "accepted response belongs to another invocation"
    );
    ensure!(
        text(&retained[4].3, "attempt_id")? == attempt.attempt_id,
        "accepted sample belongs to another registered attempt"
    );
    let sample = &retained[4].3;
    join(sample, &attempt_identity(attempt), ATTEMPT)?;
    join(
        sample,
        identity,
        &["session_id", "session_invocation_nonce"],
    )?;
    ensure!(
        sample.get("run").is_none()
            && text(sample, "economic_vector_sha256")? == result.payload.economic_vector_sha256,
        "accepted sample substituted the actual economic vector or old attempt coordinate"
    );
    // This checks acknowledgement identity, not sample qualification. The
    // canonical runner validates every numeric field before publishing ACK.
    // TODO: Qualify the composed native window/sample owners with positive native
    // execution and admitted packet capture before release/comparative reporting.
    for (_, reference, raw, _) in &retained {
        ensure!(
            root.read(reference)? == *raw,
            "acceptance evidence changed while validating"
        );
    }
    record_ref(field(message, "forwarded_from")?)
}

struct SessionOutcome {
    kind: &'static str,
    reason: Option<&'static str>,
    accepted: Vec<String>,
    active: Option<String>,
}

fn measurement_boundary(
    root: &RecordRoot,
    attempt: &PlannedAttempt,
    boundary: &'static str,
    incoming: &mut Chain,
    outgoing: &mut Chain,
    reader: &mut File,
    writer: &mut File,
    outcome: &mut SessionOutcome,
) -> Result<()> {
    ensure!(
        matches!(boundary, "ready" | "finished"),
        "unknown actual measurement boundary"
    );
    let mut marker = incoming.identity.clone();
    marker
        .as_object_mut()
        .unwrap()
        .extend(attempt_identity(attempt).as_object().unwrap().clone());
    marker
        .as_object_mut()
        .unwrap()
        .insert("boundary".to_owned(), boundary.into());
    let marker_ref = root.publish(
        &format!(
            "{}/evidence/benchmark-protocol/measurement-{boundary}.json",
            attempt.output_directory
        ),
        &canonical(&marker)?,
    )?;
    let mut payload = attempt_identity(attempt);
    payload
        .as_object_mut()
        .unwrap()
        .insert("marker".to_owned(), norito::json::to_value(&marker_ref)?);
    let (sent, expected, record) = match boundary {
        "ready" => (
            "measurement_ready",
            "measurement_begin",
            "process_observation",
        ),
        "finished" => (
            "measurement_finished",
            "measurement_recorded",
            "measurement_window",
        ),
        _ => unreachable!(),
    };
    outgoing.send(root, writer, sent, payload)?;
    let received = incoming.receive(root, reader)?;
    if text(&received, "kind")? == "stop" {
        return apply_stop(root, &received, outcome);
    }
    ensure!(
        text(&received, "kind")? == expected,
        "measurement owner acknowledged wrong boundary"
    );
    let payload = field(&received, "payload")?;
    join(payload, &attempt_identity(attempt), ATTEMPT)?;
    ensure!(
        record_ref(field(payload, "marker")?)? == marker_ref,
        "measurement acknowledgement substituted actual boundary marker"
    );
    root.read(&record_ref(field(payload, record)?)?)?;
    ensure!(
        root.read(&marker_ref)? == canonical(&marker)?,
        "measurement boundary changed before continuation"
    );
    Ok(())
}

impl SessionOutcome {
    fn interrupted() -> Self {
        Self {
            kind: "incomplete",
            reason: Some("transport_interrupted"),
            accepted: Vec::new(),
            active: None,
        }
    }
}

fn run_attempts(
    root: &RecordRoot,
    session: &SessionRequest,
    session_ref: &RecordRef,
    session_raw: &[u8],
    started_ref: &RecordRef,
    started_raw: &[u8],
    owner: &RetainedBenchmarkNetwork,
    incoming: &mut Chain,
    outgoing: &mut Chain,
    reader: &mut File,
    writer: &mut File,
    outcome: &mut SessionOutcome,
) -> Result<()> {
    let mut previous_accept = None;
    for attempt in &session.attempts {
        let raw = root.read(&attempt.request)?;
        ensure!(
            root.read(session_ref)? == session_raw && root.read(started_ref)? == started_raw,
            "session request/start changed before dispatch"
        );
        let message = incoming.receive(root, reader)?;
        if text(&message, "kind")? == "stop" {
            return apply_stop(root, &message, outcome);
        }
        validate_dispatch(
            root,
            &message,
            &incoming.identity,
            attempt,
            &raw,
            started_ref,
            previous_accept.as_ref(),
        )?;
        outcome.active = Some(attempt.attempt_id.clone());
        let request = validate_attempt_request(session, attempt, &raw)?;
        owner.inventory(&session.commit)?;
        let identity =
            BenchmarkTerminalIdentityV1::from_request(&request, attempt.request.sha256.clone());
        let started = Instant::now();
        let evidence_root = root.path.join(&attempt.output_directory).join("evidence");
        let mut boundary_failed = false;
        let mut callback = |boundary| {
            let result = measurement_boundary(
                root, attempt, boundary, incoming, outgoing, reader, writer, outcome,
            );
            boundary_failed |= result.is_err();
            result
        };
        let completion = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            match session.profile.as_str() {
                "private" => run_real_process_private_benchmark(
                    request,
                    attempt.request.sha256.clone(),
                    owner,
                    &evidence_root,
                    &mut callback,
                ),
                "transparent_control" => run_real_process_transparent_control_benchmark(
                    request,
                    attempt.request.sha256.clone(),
                    owner,
                    &evidence_root,
                    &mut callback,
                ),
                _ => unreachable!("validated profile"),
            }
        }));
        // A broken control/measurement boundary is not a settlement failure.
        // Retain the active invocation as incomplete without inventing a Rust
        // settlement terminal. Adapter/runner records classify their own failure.
        if boundary_failed {
            return Err(eyre!(
                "measurement boundary did not complete authoritatively"
            ));
        }
        let measured = match completion {
            Ok(Ok(value)) => RealProcessBenchmarkOutcomeV1::Succeeded(value),
            Ok(Err(error)) => RealProcessBenchmarkOutcomeV1::from_error(&error),
            Err(_) => RealProcessBenchmarkOutcomeV1::Failed(BenchmarkFailureReasonV1::WorkerPanic),
        };
        let successful = matches!(&measured, RealProcessBenchmarkOutcomeV1::Succeeded(_));
        let timeout = matches!(&measured, RealProcessBenchmarkOutcomeV1::TimedOut(_));
        let terminal = identity.terminal(benchmark_duration_ms(started.elapsed())?, measured)?;
        let terminal_ref = root.publish(
            &format!(
                "{}/evidence/benchmark-protocol/rust-result.json",
                attempt.output_directory
            ),
            &canonical_harness_json_bytes(&terminal)?,
        )?;
        let mut completed = attempt_identity(attempt);
        completed.as_object_mut().unwrap().insert(
            "rust_terminal".to_owned(),
            norito::json::to_value(&terminal_ref)?,
        );
        outgoing.send(root, writer, "attempt_completed", completed)?;
        // Even a successful terminal cannot advance until the owner accepts all
        // exact retained records. An unsuccessful terminal forbids acceptance.
        let response = incoming.receive(root, reader)?;
        if !successful {
            outcome.kind = if timeout { "timed_out" } else { "failed" };
            outcome.reason = Some(if timeout {
                "attempt_timed_out"
            } else {
                "attempt_failed"
            });
            ensure!(
                text(&response, "kind")? == "stop",
                "failed attempt cannot be accepted or followed by another dispatch"
            );
            return apply_stop(root, &response, outcome);
        }
        if text(&response, "kind")? == "stop" {
            return apply_stop(root, &response, outcome);
        }
        previous_accept = Some(validate_accept(
            root,
            &response,
            attempt,
            &terminal_ref,
            &incoming.identity,
        )?);
        owner.inventory(&session.commit)?;
        outcome.accepted.push(attempt.request_id.clone());
        outcome.active = None;
    }
    outcome.kind = "completed";
    outcome.reason = None;
    Ok(())
}

fn apply_stop(
    root: &RecordRoot,
    message: &HarnessJsonValue,
    outcome: &mut SessionOutcome,
) -> Result<()> {
    let payload = field(message, "payload")?;
    let active = field(payload, "active_attempt_id")?.as_str();
    ensure!(
        active == outcome.active.as_deref(),
        "stop refers to another active attempt"
    );
    let reason = text(payload, "reason")?;
    let reason = *STOP_REASONS
        .iter()
        .find(|candidate| **candidate == reason)
        .ok_or_else(|| eyre!("unknown stop reason"))?;
    if field(payload, "validation")? != &HarnessJsonValue::Null {
        root.read(&record_ref(field(payload, "validation")?)?)?;
    }
    if outcome.reason == Some("attempt_timed_out") {
        ensure!(
            reason == "attempt_timed_out",
            "owner stop contradicts typed Rust timeout"
        );
    }
    if outcome.reason == Some("attempt_failed") {
        ensure!(
            reason == "attempt_failed",
            "owner stop contradicts typed Rust failure"
        );
    }
    outcome.reason = Some(reason);
    outcome.kind = match reason {
        "attempt_timed_out" => "timed_out",
        "attempt_incomplete" | "transport_interrupted" => "incomplete",
        _ => "failed",
    };
    Err(eyre!("retained benchmark session stopped: {reason}"))
}

pub(super) fn run_from_environment() -> Result<()> {
    let required =
        |key: &str| std::env::var(key).wrap_err_with(|| format!("missing session handoff: {key}"));
    let (mut reader, mut writer) = take_control_pipes(
        required("APS_BENCHMARK_CONTROL_READ_FD")?.parse()?,
        required("APS_BENCHMARK_CONTROL_WRITE_FD")?.parse()?,
    )?;
    let root = RecordRoot::open(Path::new(&required("APS_BENCHMARK_SESSION_ROOT")?))?;
    let request_ref = root.located(
        &required("APS_BENCHMARK_SESSION_REQUEST")?,
        &required("APS_BENCHMARK_SESSION_REQUEST_SHA256")?,
    )?;
    let raw = root.read(&request_ref)?;
    let request_value = decode(&raw)?;
    let session: SessionRequest = norito::json::from_value(request_value.clone())?;
    validate_session(&session)?;
    ensure!(
        request_ref.path == format!("sessions/{}/request.json", session.session_id),
        "session request locator differs"
    );
    let mut identity = subset(&request_value, &IDENTITY[..7])?;
    identity.as_object_mut().unwrap().insert(
        "session_request_sha256".to_owned(),
        request_ref.sha256.clone().into(),
    );
    let started_ref = root.located(
        &required("APS_BENCHMARK_SESSION_STARTED")?,
        &required("APS_BENCHMARK_SESSION_STARTED_SHA256")?,
    )?;
    ensure!(
        started_ref.path == format!("sessions/{}/started.json", session.session_id),
        "session start locator differs"
    );
    let started_raw = root.read(&started_ref)?;
    let started = decode(&started_raw)?;
    let mut names = IDENTITY.to_vec();
    names.extend(["request", "command", "harness", "started_ns"]);
    exact(&started, &names)?;
    join(&started, &identity, IDENTITY)?;
    ensure!(
        number(&started, "started_ns")? > 0
            && record_ref(field(&started, "request")?)? == request_ref,
        "session start differs from request"
    );
    let mut incoming = Chain::new(identity.clone(), &started_ref.sha256, "owner_to_child")?;
    let mut outgoing = Chain::new(identity.clone(), &started_ref.sha256, "child_to_owner")?;
    let mut outcome = SessionOutcome::interrupted();
    let mut owner: Option<RetainedBenchmarkNetwork> = None;
    let execution = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut workloads = Vec::new();
        for attempt in &session.attempts {
            let request_raw = root.read(&attempt.request)?;
            let request = validate_attempt_request(&session, attempt, &request_raw)?;
            workloads.push(MatchedBenchmarkWorkloadV1::from_request(&request)?);
        }
        outcome.reason = Some("setup_failed");
        owner = Some(RetainedBenchmarkNetwork::start(&session, &workloads)?);
        let active = owner.as_ref().unwrap();
        let mut ready = identity.clone();
        let row = ready.as_object_mut().unwrap();
        row.insert(
            "network_id".to_owned(),
            norito::json::to_value(&active.network.network_id())?,
        );
        row.insert(
            "genesis_sha256".to_owned(),
            digest(
                &active
                    .network
                    .genesis()
                    .0
                    .encode_wire()
                    .map_err(|e| eyre!(e))?,
            )
            .into(),
        );
        row.insert(
            "configuration_sha256".to_owned(),
            session.configuration_sha256.clone().into(),
        );
        row.insert(
            "workload_manifest_sha256".to_owned(),
            session.workload_manifest_sha256.clone().into(),
        );
        row.insert(
            "activated_height".to_owned(),
            active.activated_height.into(),
        );
        row.insert(
            "process_inventory".to_owned(),
            norito::json::to_value(&active.inventory(&session.commit)?)?,
        );
        row.insert("worker_pid".to_owned(), std::process::id().into());
        let ports = active.network_ports_document(&identity, &session.commit)?;
        let ports_ref = root.publish(
            &format!("sessions/{}/network-ports.json", session.session_id),
            &canonical(&ports)?,
        )?;
        row.insert(
            "network_ports".to_owned(),
            norito::json::to_value(&ports_ref)?,
        );
        let ready_ref = root.publish(
            &format!("sessions/{}/ready.json", session.session_id),
            &canonical(&ready)?,
        )?;
        outgoing.send(
            &root,
            &mut writer,
            "ready",
            norito::json!({"ready":ready_ref}),
        )?;
        outcome.reason = Some("transport_interrupted");
        run_attempts(
            &root,
            &session,
            &request_ref,
            &raw,
            &started_ref,
            &started_raw,
            active,
            &mut incoming,
            &mut outgoing,
            &mut reader,
            &mut writer,
            &mut outcome,
        )?;
        ensure!(
            root.read(&request_ref)? == raw && root.read(&started_ref)? == started_raw,
            "session request/start changed during final attempt"
        );
        for attempt in &session.attempts {
            root.read(&attempt.request)?;
        }
        Ok(())
    }));
    let execution: Result<()> = match execution {
        Ok(result) => result,
        Err(_) => {
            outcome.kind = if outcome.active.is_some() {
                "incomplete"
            } else {
                "failed"
            };
            outcome.reason = Some(if outcome.active.is_some() {
                "attempt_incomplete"
            } else {
                "setup_failed"
            });
            Err(eyre!(
                "retained benchmark owner panicked outside attempt execution"
            ))
        }
    };
    let (network_shutdown, coordinator_reaped) = match owner.as_mut() {
        Some(owner) => match owner.shutdown() {
            Ok(flags) => flags,
            Err(_) => {
                outcome.kind = "failed";
                outcome.reason = Some("cleanup_failed");
                (false, false)
            }
        },
        None => (false, false),
    };
    if execution.is_err() && outcome.kind == "completed" {
        outcome.kind = "incomplete";
        outcome.reason = Some("transport_interrupted");
    }
    if execution.is_err() && outcome.reason == Some("setup_failed") {
        outcome.kind = "failed";
    }
    let mut terminal = identity;
    let row = terminal.as_object_mut().unwrap();
    row.insert("kind".to_owned(), outcome.kind.into());
    row.insert(
        "reason".to_owned(),
        norito::json::to_value(&outcome.reason)?,
    );
    row.insert(
        "accepted_request_ids".to_owned(),
        norito::json::to_value(&outcome.accepted)?,
    );
    row.insert(
        "active_attempt_id".to_owned(),
        norito::json::to_value(&outcome.active)?,
    );
    row.insert(
        "last_owner_message_sha256".to_owned(),
        incoming.previous.clone().into(),
    );
    row.insert(
        "last_worker_message_sha256".to_owned(),
        outgoing.previous.clone().into(),
    );
    row.insert(
        "network_shutdown_observed".to_owned(),
        network_shutdown.into(),
    );
    row.insert(
        "coordinator_reaped_observed".to_owned(),
        coordinator_reaped.into(),
    );
    let terminal_ref = root.publish(
        &format!("sessions/{}/worker-terminal.json", session.session_id),
        &canonical(&terminal)?,
    )?;
    outgoing.send(
        &root,
        &mut writer,
        "session_completed",
        norito::json!({"worker_terminal":terminal_ref}),
    )?;
    ensure!(
        outcome.kind == "completed" && network_shutdown && coordinator_reaped,
        "retained session did not complete successfully"
    );
    execution
}

fn verify_vector_record(
    request: &RealProcessBenchmarkRequestV1,
    request_sha256: &str,
    network_id: iroha::data_model::NetworkId,
    record: &HarnessJsonValue,
) -> Result<MatchedBenchmarkWorkloadV1> {
    let expected = matched_workload_record(request, request_sha256, network_id)?;
    ensure!(
        canonical_harness_json_bytes(record)? == canonical_harness_json_bytes(&expected)?,
        "economic record differs from native request-derived vector, canonical Norito bytes or complete bindings"
    );
    MatchedBenchmarkWorkloadV1::from_request(request)
}

/// Offline economic verification through the exact compiled harness/model codecs.
/// No validator, coordinator, pipe controller or proof generator is started here.
pub(super) fn verify_vector_from_environment() -> Result<()> {
    let required = |key: &str| {
        std::env::var(key).wrap_err_with(|| format!("missing native vector handoff: {key}"))
    };
    let root = RecordRoot::open(Path::new(&required("APS_BENCHMARK_SESSION_ROOT")?))?;
    let verification_ref = root.located(
        &required("APS_BENCHMARK_VECTOR_REQUEST")?,
        &required("APS_BENCHMARK_VECTOR_REQUEST_SHA256")?,
    )?;
    let verification_raw = root.read(&verification_ref)?;
    let verification = decode(&verification_raw)?;
    exact(
        &verification,
        &[
            "version",
            "protocol",
            "kind",
            "session_request",
            "benchmark_request",
            "ready",
            "workload_record",
        ],
    )?;
    ensure!(
        number(&verification, "version")? == 1
            && text(&verification, "protocol")? == PROTOCOL
            && text(&verification, "kind")? == "benchmark_economic_vector_verification",
        "unknown native vector verification contract"
    );
    let mut inputs = Vec::new();
    for name in [
        "session_request",
        "benchmark_request",
        "ready",
        "workload_record",
    ] {
        let reference = record_ref(field(&verification, name)?)?;
        let raw = root.read(&reference)?;
        let value = decode(&raw)?;
        inputs.push((name, reference, raw, value));
    }
    let session: SessionRequest = norito::json::from_value(inputs[0].3.clone())?;
    validate_session(&session)?;
    ensure!(
        inputs[0].1.path == format!("sessions/{}/request.json", session.session_id),
        "native verifier substituted session request locator"
    );
    let selected = session
        .attempts
        .iter()
        .filter(|attempt| attempt.request == inputs[1].1)
        .collect::<Vec<_>>();
    ensure!(
        selected.len() == 1,
        "native verifier request is absent or duplicated in registered session"
    );
    let attempt = selected[0];
    let request = validate_attempt_request(&session, attempt, &inputs[1].2)?;
    let prefix = format!("{}/evidence/benchmark-protocol", attempt.output_directory);
    ensure!(
        verification_ref.path == format!("{prefix}/economic-vector-verification-request.json")
            && inputs[2].1.path == format!("sessions/{}/ready.json", session.session_id)
            && inputs[3].1.path
                == format!(
                    "{}/evidence/matched-workload.json",
                    attempt.output_directory
                ),
        "native verification input locator differs from registered attempt"
    );
    let mut identity = subset(&inputs[0].3, &IDENTITY[..7])?;
    identity.as_object_mut().unwrap().insert(
        "session_request_sha256".to_owned(),
        inputs[0].1.sha256.clone().into(),
    );
    let ready = &inputs[2].3;
    let mut ready_fields = IDENTITY.to_vec();
    ready_fields.extend([
        "network_id",
        "genesis_sha256",
        "configuration_sha256",
        "workload_manifest_sha256",
        "activated_height",
        "process_inventory",
        "worker_pid",
        "network_ports",
    ]);
    exact(ready, &ready_fields)?;
    join(ready, &identity, IDENTITY)?;
    ensure!(
        text(ready, "configuration_sha256")? == session.configuration_sha256
            && text(ready, "workload_manifest_sha256")? == session.workload_manifest_sha256
            && number(ready, "activated_height")? > 0
            && number(ready, "worker_pid")? > 1,
        "native verifier ready identity differs"
    );
    checked_digest(text(ready, "genesis_sha256")?)?;
    let network_id = norito::json::from_value::<iroha::data_model::NetworkId>(
        field(ready, "network_id")?.clone(),
    )?;
    let ports_ref = record_ref(field(ready, "network_ports")?)?;
    ensure!(
        ports_ref.path == format!("sessions/{}/network-ports.json", session.session_id),
        "native verifier session ports locator differs"
    );
    let ports_raw = root.read(&ports_ref)?;
    let ports = decode(&ports_raw)?;
    let inventory: Vec<RealProcessInventoryRowV1> =
        norito::json::from_value(field(ready, "process_inventory")?.clone())?;
    validate_network_ports_document(
        &ports,
        &identity,
        network_id,
        TopologyShape::new(session.participants),
        &inventory,
    )?;
    let workload = verify_vector_record(&request, &inputs[1].1.sha256, network_id, &inputs[3].3)?;
    let canonical_vector = norito::encode_canonical(&workload)?;
    let mut output = identity;
    let map = output.as_object_mut().unwrap();
    map.extend(attempt_identity(attempt).as_object().unwrap().clone());
    map.insert(
        "kind".to_owned(),
        "benchmark_economic_vector_verified".into(),
    );
    map.insert("verified".to_owned(), true.into());
    map.insert(
        "request_sha256".to_owned(),
        inputs[1].1.sha256.clone().into(),
    );
    map.insert(
        "network_id".to_owned(),
        norito::json::to_value(&network_id)?,
    );
    map.insert(
        "workload_manifest_sha256".to_owned(),
        session.workload_manifest_sha256.clone().into(),
    );
    map.insert(
        "economic_vector_sha256".to_owned(),
        workload.digest()?.into(),
    );
    map.insert(
        "canonical_economic_vector_sha256".to_owned(),
        digest(&canonical_vector).into(),
    );
    map.insert(
        "primary_payment_count".to_owned(),
        session.participants.into(),
    );
    map.insert(
        "monetary_movement_count".to_owned(),
        (session.participants + 1).into(),
    );
    map.insert(
        "verification_request".to_owned(),
        norito::json::to_value(&verification_ref)?,
    );
    for (name, reference, raw, _) in &inputs {
        ensure!(
            root.read(reference)? == *raw,
            "native vector input changed during verification"
        );
        map.insert((*name).to_owned(), norito::json::to_value(reference)?);
    }
    ensure!(
        root.read(&verification_ref)? == verification_raw,
        "native verification request changed"
    );
    ensure!(
        root.read(&ports_ref)? == ports_raw,
        "native verifier endpoint inputs changed"
    );
    root.publish(
        &format!("{prefix}/economic-vector-verification.json"),
        &canonical(&output)?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn planned() -> SessionRequest {
        planned_with_warmups(5)
    }

    fn planned_with_warmups(warmups: u64) -> SessionRequest {
        let mut request = SessionRequest {
            version: 1,
            protocol: (PROTOCOL.to_owned()),
            kind: "benchmark_session".to_owned(),
            scope_sha256: ("1".repeat(64)),
            campaign_id: "trial-a".to_owned(),
            plan_sha256: ("2".repeat(64)),
            session_id: ("3".repeat(64)),
            session_invocation_nonce: ("4".repeat(64)),
            workload_manifest_sha256: digest(&canonical(&policy(3)).unwrap()),
            workload_manifest: policy(3),
            commit: ("5".repeat(40)),
            configuration_sha256: ("6".repeat(64)),
            profile: "private".to_owned(),
            participants: 3,
            seed: 9,
            warmups,
            attempts: Vec::new(),
        };
        for index in 0..warmups + 2 {
            request.attempts.push(PlannedAttempt {
                attempt_id: ("1".repeat(64)),
                request_id: ("2".repeat(64)),
                invocation_nonce: format!("{index:064x}"),
                session_attempt_index: index,
                request: RecordRef {
                    path: "placeholder".to_owned(),
                    sha256: ("7".repeat(64)),
                    bytes: 1,
                },
                output_directory: "placeholder".to_owned(),
            });
        }
        request.session_id = session_id(&request).unwrap();
        for index in 0..request.attempts.len() {
            let job = norito::json!({"kind":"benchmark","profile":(request.profile.clone()),"participants":(request.participants),"seed":(request.seed),
                "session_id":(request.session_id.clone()),"session_attempt_index":index,"warmup":((index as u64) < warmups),
                "configuration_sha256":(request.configuration_sha256.clone()),"workload_manifest_sha256":(request.workload_manifest_sha256.clone())});
            let id = digest(&canonical(&job).unwrap());
            let attempt_id = digest(&canonical(&norito::json!({"domain":"iroha:private-settlement:registered-attempt:v1",
                "scope_sha256":(request.scope_sha256.clone()),"campaign_id":(request.campaign_id.clone()),
                "plan_sha256":(request.plan_sha256.clone()),"request_id":(id.clone())})).unwrap());
            let output = format!("attempts/{:05}-{id}", index + 1);
            request.attempts[index] = PlannedAttempt {
                attempt_id,
                request_id: id,
                invocation_nonce: format!("{:064x}", index + 1),
                session_attempt_index: index as u64,
                request: RecordRef {
                    path: format!("{output}/request.json"),
                    sha256: ("7".repeat(64)),
                    bytes: 1,
                },
                output_directory: output,
            };
        }
        request
    }

    fn identity() -> HarnessJsonValue {
        norito::json!({"version":1,"protocol":PROTOCOL,"scope_sha256":("1".repeat(64)),"campaign_id":"trial-a",
            "plan_sha256":("2".repeat(64)),"session_id":("3".repeat(64)),"session_invocation_nonce":("4".repeat(64)),"session_request_sha256":("5".repeat(64))})
    }

    fn root() -> (tempfile::TempDir, RecordRoot) {
        let temporary = tempfile::tempdir().unwrap();
        fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let root = RecordRoot::open(&temporary.path().canonicalize().unwrap()).unwrap();
        (temporary, root)
    }

    fn mkdir(root: &RecordRoot, name: &str) {
        fs::create_dir_all(root.path.join(name)).unwrap();
        let mut path = root.path.clone();
        for part in name.split('/') {
            path.push(part);
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
    }

    #[test]
    fn retained_session_plan_binds_configured_warmups_and_every_attempt() {
        let request = planned();
        validate_session(&request).unwrap();
        assert_eq!(request.attempts.len(), 7);
        assert_eq!(request.warmups, 5);
        for warmups in [6, 1000] {
            validate_session(&planned_with_warmups(warmups)).unwrap();
        }
        for field in [
            "missing",
            "reordered",
            "nonce",
            "policy",
            "configuration",
            "warmups",
            "profile",
            "request",
            "attempt",
            "locator",
        ] {
            let mut changed = planned();
            match field {
                "missing" => {
                    changed.attempts.pop();
                }
                "reordered" => changed.attempts.swap(1, 2),
                "nonce" => {
                    changed.attempts[1].invocation_nonce =
                        changed.attempts[0].invocation_nonce.clone()
                }
                "policy" => {
                    changed
                        .workload_manifest
                        .as_object_mut()
                        .unwrap()
                        .insert("sponsor_reimbursement_amount".to_owned(), 0.into());
                }
                "configuration" => changed.configuration_sha256 = "8".repeat(64),
                "warmups" => changed.warmups = 6,
                "profile" => changed.profile = "transparent_control".to_owned(),
                "request" => changed.attempts[6].request_id = "9".repeat(64),
                "attempt" => changed.attempts[6].attempt_id = "9".repeat(64),
                "locator" => {
                    changed.attempts[6].output_directory =
                        changed.attempts[0].output_directory.clone()
                }
                _ => unreachable!(),
            }
            assert!(validate_session(&changed).is_err(), "accepted {field}");
        }
        for invalid in [0, 1, 4, 1001, u64::MAX] {
            let mut changed = planned();
            changed.warmups = invalid;
            assert!(validate_session(&changed).is_err());
        }
        let mut unicode = planned();
        unicode.attempts[0].output_directory = format!("attempts/12345é{}", "x".repeat(63));
        let outcome = std::panic::catch_unwind(|| validate_session(&unicode));
        assert!(
            matches!(outcome, Ok(Err(_))),
            "untrusted Unicode locator must reject without panic"
        );
        for field in ["warmups", "run", "unexpected"] {
            let mut value = norito::json::to_value(&planned()).unwrap();
            value
                .as_object_mut()
                .unwrap()
                .insert(field.to_owned(), true.into());
            assert!(norito::json::from_value::<SessionRequest>(value).is_err());
        }
    }

    #[test]
    fn retained_session_control_rejects_duplicate_noncanonical_or_unbounded_json() {
        assert_eq!(
            canonical(&decode(br#"{"a":1,"b":true}"#).unwrap()).unwrap(),
            br#"{"a":1,"b":true}"#
        );
        for raw in [
            br#"{"a":1,"a":1}"#.as_slice(),
            br#"{"a":1.0}"#,
            br#"{"a":-1}"#,
            br#"{"a":18446744073709551616}"#,
            br#"{ "a":1}"#,
            b"{}\n",
            br#"{"a":1e999}"#,
        ] {
            assert!(decode(raw).is_err());
        }
        let mut nested = HarnessJsonValue::Null;
        for _ in 0..34 {
            nested = HarnessJsonValue::Array(vec![nested]);
        }
        assert!(canonical(&nested).is_err());
        for path in ["", ".", "../x", "/x", "a//b", "a/./b", "a\\b", "a\nb"] {
            assert!(relative(path).is_err());
        }
        assert!(
            record_ref(&norito::json!({"path":"safe","sha256":("1".repeat(64)),"bytes":true}))
                .is_err()
        );
    }

    #[test]
    fn retained_session_records_require_exact_bytes_and_single_owner_inode() {
        let (_temporary, root) = root();
        let reference = root.publish("record.json", b"{} ").unwrap();
        assert_eq!(root.read(&reference).unwrap(), b"{} ");
        assert!(root.publish("record.json", b"{}").is_err());
        fs::write(root.path.join("record.json"), b" []").unwrap();
        assert!(root.read(&reference).is_err());
        let good = root.publish("good.json", b"{}").unwrap();
        fs::hard_link(root.path.join("good.json"), root.path.join("linked.json")).unwrap();
        assert!(root.read(&good).is_err());
        let good = root.publish("unlinked.json", b"{}").unwrap();
        fs::rename(root.path.join("unlinked.json"), root.path.join("held.json")).unwrap();
        std::os::unix::fs::symlink("held.json", root.path.join("unlinked.json")).unwrap();
        assert!(root.read(&good).is_err());
    }

    #[test]
    fn retained_session_records_reject_parent_link_and_owner_permission_change() {
        let (_temporary, root) = root();
        mkdir(&root, "records");
        let reference = root.publish("records/result.json", b"{}").unwrap();
        fs::rename(root.path.join("records"), root.path.join("prior")).unwrap();
        std::os::unix::fs::symlink("prior", root.path.join("records")).unwrap();
        assert!(root.read(&reference).is_err());
        let reference = root.publish("plain.json", b"{}").unwrap();
        fs::set_permissions(
            root.path.join("plain.json"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(root.read(&reference).is_err());
        fs::set_permissions(&root.path, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(root.validate().is_err());
    }

    #[test]
    fn retained_session_control_publishes_exact_frame_before_pipe_write() {
        struct BeforeWrite<'a> {
            root: &'a RecordRoot,
            path: String,
            seen: bool,
        }
        impl Write for BeforeWrite<'_> {
            fn write(&mut self, raw: &[u8]) -> std::io::Result<usize> {
                assert_eq!(fs::read(self.root.path.join(&self.path)).unwrap(), raw);
                self.seen = true;
                Ok(raw.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let (_temporary, root) = root();
        let mut chain = Chain::new(identity(), &"6".repeat(64), "child_to_owner").unwrap();
        mkdir(&root, &chain.prefix);
        let ready = root.publish("ready.json", b"{}").unwrap();
        let mut writer = BeforeWrite {
            root: &root,
            path: (chain.path("frame")),
            seen: false,
        };
        let reference = chain
            .send(&root, &mut writer, "ready", norito::json!({"ready":ready}))
            .unwrap();
        assert!(writer.seen);
        assert_eq!(chain.previous, reference.sha256);
        assert_eq!(chain.sequence, 1);
    }

    #[test]
    fn retained_session_control_partial_frame_poison_is_permanent() {
        let (_temporary, root) = root();
        let mut chain = Chain::new(identity(), &"6".repeat(64), "owner_to_child").unwrap();
        mkdir(&root, &chain.prefix);
        let raw = vec![0, 0, 0, 10, b'{'];
        assert!(chain.receive(&root, &mut Cursor::new(raw.clone())).is_err());
        assert!(chain.poisoned);
        assert_eq!(
            fs::read(root.path.join(chain.path("incomplete"))).unwrap(),
            raw
        );
        assert!(chain.receive(&root, &mut Cursor::new(Vec::new())).is_err());
        assert_eq!(chain.sequence, 0);
    }

    #[test]
    fn retained_session_control_failed_write_cannot_be_retried() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let (_temporary, root) = root();
        let mut chain = Chain::new(identity(), &"6".repeat(64), "child_to_owner").unwrap();
        mkdir(&root, &chain.prefix);
        let ready = root.publish("ready.json", b"{}").unwrap();
        assert!(
            chain
                .send(&root, &mut Broken, "ready", norito::json!({"ready":ready}))
                .is_err()
        );
        assert!(chain.poisoned);
        assert!(root.path.join(chain.path("frame")).is_file());
        assert!(
            chain
                .send(
                    &root,
                    &mut Vec::new(),
                    "ready",
                    norito::json!({"ready":ready})
                )
                .is_err()
        );
    }

    #[test]
    fn retained_session_inherited_pipes_are_direction_exact_and_close_on_exec() {
        use std::os::fd::IntoRawFd;
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let read_fd = child.stdout.take().unwrap().into_raw_fd();
        let write_fd = child.stdin.take().unwrap().into_raw_fd();
        let (mut reader, mut writer) = take_control_pipes(read_fd, write_fd).unwrap();
        assert!(
            rustix::io::fcntl_getfd(&reader)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        assert!(
            rustix::io::fcntl_getfd(&writer)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        writer.write_all(b"bound").unwrap();
        let mut observed = [0; 5];
        reader.read_exact(&mut observed).unwrap();
        assert_eq!(&observed, b"bound");
        drop(writer);
        drop(reader);
        assert!(child.wait().unwrap().success());
        assert!(take_control_pipes(0, 1).is_err());
        assert!(take_control_pipes(9, 9).is_err());
    }

    #[test]
    fn retained_session_dispatch_requires_exact_durable_start_and_consumed_predecessor() {
        for change in [
            "none",
            "preceding",
            "ordinal",
            "index",
            "nonce",
            "start-missing",
        ] {
            let (_temporary, root) = root();
            let session = planned();
            let mut attempt = session.attempts.into_iter().nth(1).unwrap();
            mkdir(&root, &attempt.output_directory);
            let raw = br#"{"kind":"boundary-test-only"}"#;
            attempt.request = root
                .publish(&format!("{}/request.json", attempt.output_directory), raw)
                .unwrap();
            let started_ref = root.publish("session-started.json", b"{}").unwrap();
            let previous = root
                .publish("prior-accept.frame", b"bound upstream acceptance")
                .unwrap();
            let other = root
                .publish("other-accept.frame", b"different upstream acceptance")
                .unwrap();
            let identity = identity();
            let mut start = identity.clone();
            start
                .as_object_mut()
                .unwrap()
                .extend(attempt_identity(&attempt).as_object().unwrap().clone());
            let map = start.as_object_mut().unwrap();
            map.insert("ordinal".to_owned(), 2.into());
            map.insert(
                "session_started".to_owned(),
                norito::json::to_value(&started_ref).unwrap(),
            );
            map.insert(
                "request".to_owned(),
                norito::json::to_value(&attempt.request).unwrap(),
            );
            map.insert("outer_timeout_ms".to_owned(), 300000.into());
            map.insert("started_ns".to_owned(), 1.into());
            map.insert(
                "preceding_acceptance".to_owned(),
                norito::json::to_value(&previous).unwrap(),
            );
            match change {
                "preceding" => {
                    map.insert(
                        "preceding_acceptance".to_owned(),
                        norito::json::to_value(&other).unwrap(),
                    );
                }
                "ordinal" => {
                    map.insert("ordinal".to_owned(), 3.into());
                }
                "index" => {
                    map.insert("session_attempt_index".to_owned(), 2.into());
                }
                "nonce" => {
                    map.insert("invocation_nonce".to_owned(), "9".repeat(64).into());
                }
                _ => {}
            }
            let start_ref = root
                .publish(
                    &format!("{}/started.json", attempt.output_directory),
                    &canonical(&start).unwrap(),
                )
                .unwrap();
            if change == "start-missing" {
                fs::remove_file(root.path.join(&start_ref.path)).unwrap();
            }
            let mut payload = attempt_identity(&attempt);
            payload.as_object_mut().unwrap().insert(
                "request".to_owned(),
                norito::json::to_value(&attempt.request).unwrap(),
            );
            payload.as_object_mut().unwrap().insert(
                "attempt_started".to_owned(),
                norito::json::to_value(&start_ref).unwrap(),
            );
            let message = norito::json!({"kind":"dispatch","payload":payload});
            assert_eq!(
                validate_dispatch(
                    &root,
                    &message,
                    &identity,
                    &attempt,
                    raw,
                    &started_ref,
                    Some(&previous)
                )
                .is_ok(),
                change == "none",
                "{change}"
            );
        }
    }

    #[test]
    fn retained_session_accept_rejects_failure_timeout_or_substituted_validated_records() {
        for change in [
            "none",
            "failed",
            "timed_out",
            "validation",
            "response",
            "sample",
            "adapter",
            "vector",
            "session",
            "old-run",
        ] {
            let (_temporary, root) = root();
            let mut attempt = planned().attempts.remove(0);
            let (terminal_identity, success) = benchmark_terminal_fixture();
            attempt.request_id = terminal_identity.request_id.clone();
            attempt.invocation_nonce = terminal_identity.invocation_nonce.clone();
            attempt.request.sha256 = terminal_identity.request_sha256.clone();
            let outcome = match change {
                "failed" => {
                    RealProcessBenchmarkOutcomeV1::Failed(BenchmarkFailureReasonV1::ExecutionError)
                }
                "timed_out" => RealProcessBenchmarkOutcomeV1::TimedOut(BenchmarkDeadlineV1 {
                    stage: BenchmarkDeadlineStageV1::PrivateReceipt,
                    budget_ms: 10,
                    elapsed_ms: 11,
                }),
                _ => RealProcessBenchmarkOutcomeV1::Succeeded(success),
            };
            let terminal = root
                .publish(
                    "terminal.json",
                    &canonical_harness_json_bytes(
                        &terminal_identity.terminal(12, outcome).unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap();
            let adapter=root.publish("adapter.json",&canonical(&norito::json!({"status":(if change == "adapter" {"failed"} else {"succeeded"}),
                "request_id":(attempt.request_id.clone()),"invocation_nonce":(attempt.invocation_nonce.clone()),"request_sha256":(attempt.request.sha256.clone()),"rust_terminal":terminal})).unwrap()).unwrap();
            let response=root.publish("response.json",&canonical(&norito::json!({"request_id":(if change == "response" {"9".repeat(64)} else {attempt.request_id.clone()}),
                "invocation_nonce":(attempt.invocation_nonce.clone()),"request_sha256":(attempt.request.sha256.clone())})).unwrap()).unwrap();
            let identity = identity();
            let mut sample_value = attempt_identity(&attempt);
            let map = sample_value.as_object_mut().unwrap();
            map.extend(
                subset(&identity, &["session_id", "session_invocation_nonce"])
                    .unwrap()
                    .as_object()
                    .unwrap()
                    .clone(),
            );
            map.insert(
                "economic_vector_sha256".to_owned(),
                (if change == "vector" {
                    "8".repeat(64)
                } else {
                    "a".repeat(64)
                })
                .into(),
            );
            map.insert("stages_ms".to_owned(), norito::json!({"end_to_end":1.5}));
            if change == "sample" {
                map.insert("attempt_id".to_owned(), "9".repeat(64).into());
            }
            if change == "session" {
                map.insert("session_id".to_owned(), "9".repeat(64).into());
            }
            if change == "old-run" {
                map.insert("run".to_owned(), 0.into());
            }
            let sample = root
                .publish(
                    "sample.json",
                    &canonical_harness_json_bytes(&sample_value).unwrap(),
                )
                .unwrap();
            let mut validation =
                subset(&identity, &["scope_sha256", "campaign_id", "plan_sha256"]).unwrap();
            let map = validation.as_object_mut().unwrap();
            map.insert("request_id".to_owned(), attempt.request_id.clone().into());
            map.insert("attempt_id".to_owned(), attempt.attempt_id.clone().into());
            map.insert("passed".to_owned(), (change != "validation").into());
            map.insert("validation_kind".to_owned(), "accepted".into());
            map.insert(
                "response".to_owned(),
                norito::json::to_value(&response).unwrap(),
            );
            map.insert(
                "sample".to_owned(),
                norito::json::to_value(&sample).unwrap(),
            );
            let validation = root
                .publish("validation.json", &canonical(&validation).unwrap())
                .unwrap();
            let previous = root
                .publish("owner-accept.frame", b"exact prior owner frame")
                .unwrap();
            let mut payload = attempt_identity(&attempt);
            for (key, reference) in [
                ("rust_terminal", &terminal),
                ("adapter_outcome", &adapter),
                ("response", &response),
                ("validation", &validation),
                ("sample", &sample),
            ] {
                payload
                    .as_object_mut()
                    .unwrap()
                    .insert(key.to_owned(), norito::json::to_value(reference).unwrap());
            }
            let message =
                norito::json!({"kind":"accept","payload":payload,"forwarded_from":previous});
            assert_eq!(
                validate_accept(&root, &message, &attempt, &terminal, &identity).is_ok(),
                change == "none",
                "{change}"
            );
        }
    }

    #[test]
    fn retained_session_stop_preserves_accepted_prefix_and_rejects_timeout_relabel() {
        let (_temporary, root) = root();
        for reason in STOP_REASONS {
            let mut outcome = SessionOutcome {
                kind: "incomplete",
                reason: Some("transport_interrupted"),
                accepted: vec!["1".repeat(64), "2".repeat(64)],
                active: Some("3".repeat(64)),
            };
            let message = norito::json!({"payload":{"active_attempt_id":("3".repeat(64)),"reason":reason,"validation":null}});
            assert!(apply_stop(&root, &message, &mut outcome).is_err());
            assert_eq!(outcome.accepted, vec!["1".repeat(64), "2".repeat(64)]);
            assert_eq!(outcome.reason, Some(*reason));
        }
        let mut timeout = SessionOutcome {
            kind: "timed_out",
            reason: Some("attempt_timed_out"),
            accepted: Vec::new(),
            active: Some("3".repeat(64)),
        };
        let message = norito::json!({"payload":{"active_attempt_id":("3".repeat(64)),"reason":"attempt_failed","validation":null}});
        assert!(apply_stop(&root, &message, &mut timeout).is_err());
        assert_eq!(timeout.reason, Some("attempt_timed_out"));
    }

    #[test]
    fn retained_session_measurement_boundaries_are_local_exact_and_ordered() {
        let identity = identity();
        let marker = norito::json!({"path":"marker.json","sha256":("a".repeat(64)),"bytes":2});
        for (kind, direction, extra) in [
            ("measurement_ready", "child_to_owner", None),
            ("measurement_finished", "child_to_owner", None),
            (
                "measurement_begin",
                "owner_to_child",
                Some("process_observation"),
            ),
            (
                "measurement_recorded",
                "owner_to_child",
                Some("measurement_window"),
            ),
        ] {
            let mut message = identity.clone();
            let mut payload = attempt_identity(&planned().attempts[0]);
            payload
                .as_object_mut()
                .unwrap()
                .insert("marker".to_owned(), marker.clone());
            if let Some(extra) = extra {
                payload
                    .as_object_mut()
                    .unwrap()
                    .insert(extra.to_owned(), marker.clone());
            }
            let map = message.as_object_mut().unwrap();
            map.insert("channel".to_owned(), "adapter_worker".into());
            map.insert("direction".to_owned(), direction.into());
            map.insert("sequence".to_owned(), 0.into());
            map.insert("previous_message_sha256".to_owned(), "b".repeat(64).into());
            map.insert("kind".to_owned(), kind.into());
            map.insert("payload".to_owned(), payload);
            map.insert("forwarded_from".to_owned(), HarnessJsonValue::Null);
            validate_message(&message, &identity, direction).unwrap();
            message
                .as_object_mut()
                .unwrap()
                .insert("forwarded_from".to_owned(), marker.clone());
            assert!(validate_message(&message, &identity, direction).is_err());
        }
    }

    #[test]
    fn native_economic_vector_verifier_rederives_full_norito_and_json_record() {
        let request = benchmark_terminal_request_fixture();
        let network = iroha::data_model::NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(hash(0xe8)),
        );
        let record = matched_workload_record(&request, &"a".repeat(64), network).unwrap();
        let workload = verify_vector_record(&request, &"a".repeat(64), network, &record).unwrap();
        assert_eq!(
            text(&record, "economic_vector_sha256").unwrap(),
            workload.digest().unwrap()
        );
        assert_eq!(
            hex::decode(text(&record, "canonical_economic_vector_hex").unwrap()).unwrap(),
            norito::encode_canonical(&workload).unwrap()
        );
        for key in [
            "request_id",
            "invocation_nonce",
            "request_sha256",
            "configuration_sha256",
            "economic_vector_sha256",
            "canonical_economic_vector_hex",
            "workload_manifest_sha256",
            "unexpected",
        ] {
            let mut changed = record.clone();
            changed
                .as_object_mut()
                .unwrap()
                .insert(key.to_owned(), "0".repeat(64).into());
            assert!(
                verify_vector_record(&request, &"a".repeat(64), network, &changed).is_err(),
                "accepted {key}"
            );
        }
        for key in [
            "seed",
            "session_attempt_index",
            "participants",
            "warmup",
            "sponsor",
            "payments",
            "reimbursement",
        ] {
            let mut changed = record.clone();
            changed
                .as_object_mut()
                .unwrap()
                .get_mut("workload")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.to_owned(), HarnessJsonValue::Null);
            assert!(
                verify_vector_record(&request, &"a".repeat(64), network, &changed).is_err(),
                "accepted changed {key}"
            );
        }
        let other_network = iroha::data_model::NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(hash(0xe9)),
        );
        assert!(verify_vector_record(&request, &"a".repeat(64), other_network, &record).is_err());
    }

    #[test]
    fn native_economic_vector_verifier_rejects_rehashed_alternate_coordinates() {
        let request = benchmark_terminal_request_fixture();
        let network = iroha::data_model::NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(hash(0xe8)),
        );
        for coordinate in ["seed", "index", "warmup", "participants"] {
            let mut changed = benchmark_terminal_request_fixture();
            match coordinate {
                "seed" => changed.seed += 1,
                "index" => changed.session_attempt_index += 1,
                "warmup" => changed.payload.warmup = !changed.payload.warmup,
                "participants" => changed.participants = 4,
                _ => unreachable!(),
            }
            // Every JSON, Norito byte and digest agrees with the alternate
            // coordinates; the exact registered request still rejects it.
            let alternate = matched_workload_record(&changed, &"a".repeat(64), network).unwrap();
            assert!(verify_vector_record(&request, &"a".repeat(64), network, &alternate).is_err());
        }
    }
    #[test]
    fn retained_session_endpoint_manifest_joins_every_process_and_group() {
        for participants in [2, 3, 4, 8, 16] {
            let (document, identity, network, shape, inventory) = endpoint_fixture(participants);
            validate_network_ports_document(&document, &identity, network, shape, &inventory)
                .unwrap();
            let raw = canonical(&document).unwrap();
            assert_eq!(decode(&raw).unwrap(), document);
        }
    }

    #[test]
    fn retained_session_endpoint_manifest_rejects_rebound_missing_or_wrong_scope_rows() {
        let (document, identity, network, shape, inventory) = endpoint_fixture(3);
        for key in [
            "pid",
            "peer_index",
            "role",
            "dataspace_ordinal",
            "validator_ordinal",
            "unexpected",
        ] {
            let mut changed = document.clone();
            changed
                .as_object_mut()
                .unwrap()
                .get_mut("peers")
                .unwrap()
                .as_array_mut()
                .unwrap()[15]
                .as_object_mut()
                .unwrap()
                .insert(key.to_owned(), HarnessJsonValue::Null);
            assert!(
                validate_network_ports_document(&changed, &identity, network, shape, &inventory)
                    .is_err()
            );
        }
        for (kind, key, value) in [
            ("torii", "address", "0.0.0.0"),
            ("torii", "transport", "udp"),
            ("p2p", "visibility", "public"),
        ] {
            let mut changed = document.clone();
            changed
                .as_object_mut()
                .unwrap()
                .get_mut("peers")
                .unwrap()
                .as_array_mut()
                .unwrap()[15]
                .as_object_mut()
                .unwrap()
                .get_mut(kind)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.to_owned(), value.into());
            assert!(
                validate_network_ports_document(&changed, &identity, network, shape, &inventory)
                    .is_err()
            );
        }
        for action in ["missing", "reordered", "zero", "overlap", "groups"] {
            let mut changed = document.clone();
            let peers = changed
                .as_object_mut()
                .unwrap()
                .get_mut("peers")
                .unwrap()
                .as_array_mut()
                .unwrap();
            match action {
                "missing" => {
                    peers.pop();
                }
                "reordered" => peers.swap(0, 15),
                "zero" | "overlap" => {
                    peers[15]
                        .as_object_mut()
                        .unwrap()
                        .get_mut("torii")
                        .unwrap()
                        .as_object_mut()
                        .unwrap()
                        .insert(
                            "port".to_owned(),
                            (if action == "zero" { 0_u64 } else { 10000 }).into(),
                        );
                }
                "groups" => {
                    changed
                        .as_object_mut()
                        .unwrap()
                        .insert("groups".to_owned(), norito::json!({}));
                }
                _ => unreachable!(),
            }
            assert!(
                validate_network_ports_document(&changed, &identity, network, shape, &inventory)
                    .is_err()
            );
        }
        let mut other = inventory.clone();
        other[16].pid += 100;
        assert!(
            validate_network_ports_document(&document, &identity, network, shape, &other).is_err()
        );
    }

    fn endpoint_fixture(
        participants: usize,
    ) -> (
        HarnessJsonValue,
        HarnessJsonValue,
        iroha::data_model::NetworkId,
        TopologyShape,
        Vec<RealProcessInventoryRowV1>,
    ) {
        let identity = identity();
        let network = iroha::data_model::NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(hash(0xef)),
        );
        let shape = TopologyShape::new(participants);
        let mut inventory = vec![RealProcessInventoryRowV1 {
            role: "coordinator".to_owned(),
            dataspace_ordinal: None,
            validator_ordinal: None,
            pid: 10,
            executable_sha256: "a".repeat(64),
            revision: "b".repeat(40),
            health_observed: true,
        }];
        let mut peers = Vec::new();
        let mut torii_ports = Vec::new();
        let mut public_p2p_ports = Vec::new();
        let mut restricted_p2p_ports = Vec::new();
        for index in 0..shape.process_count() {
            let lane = index / VALIDATORS_PER_LANE;
            let role = if lane == 0 {
                "global_validator"
            } else {
                "dataspace_validator"
            };
            let dataspace = (lane != 0).then(|| (lane - 1) as u64);
            let validator = Some((index % VALIDATORS_PER_LANE) as u64);
            let pid = 100 + index as u32;
            let torii = 10000 + index as u16;
            let p2p = 20000 + index as u16;
            let visibility =
                if lane == 0 || shape.participant_visibility(lane - 1) == LaneVisibility::Public {
                    "public"
                } else {
                    "restricted"
                };
            torii_ports.push(torii);
            if visibility == "public" {
                public_p2p_ports.push(p2p);
            } else {
                restricted_p2p_ports.push(p2p);
            }
            inventory.push(RealProcessInventoryRowV1 {
                role: role.to_owned(),
                dataspace_ordinal: dataspace,
                validator_ordinal: validator,
                pid,
                executable_sha256: "a".repeat(64),
                revision: "b".repeat(40),
                health_observed: true,
            });
            peers.push(norito::json!({"peer_index":index,"pid":pid,"role":role,"dataspace_ordinal":dataspace,"validator_ordinal":validator,
                "torii":{"address":"127.0.0.1","port":torii,"transport":"tcp"},
                "p2p":{"address":"127.0.0.1","port":p2p,"transport":"tcp","visibility":visibility}}));
        }
        let mut document = identity.clone();
        let map = document.as_object_mut().unwrap();
        map.insert("kind".to_owned(), "benchmark_network_ports".into());
        map.insert(
            "network_id".to_owned(),
            norito::json::to_value(&network).unwrap(),
        );
        map.insert("participants".to_owned(), participants.into());
        map.insert(
            "groups".to_owned(),
            norito::json::to_value(&LeakagePortManifestV1 {
                version: 1,
                torii_ports,
                public_p2p_ports,
                restricted_p2p_ports,
            })
            .unwrap(),
        );
        map.insert("peers".to_owned(), HarnessJsonValue::Array(peers));
        (document, identity, network, shape, inventory)
    }
}

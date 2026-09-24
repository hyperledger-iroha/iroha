//! Read every physical event of the original, independently pinned collector journal.
//!
//! This owner establishes original signed requests and collector-log consistency only. Resource
//! capture references and reported Applied observations do not authenticate resource or execution
//! evidence. The ordinary proof verifier still authenticates the complete canonical transcript.
//! The facts filesystem owner retains and rechecks the original journal descriptor,
//! digest and ancestors through facts publication. This in-memory reader borrows
//! that authority and does not replace its retained lease.

use super::*;
use norito::json::{self, Value};

mod events;
mod schedule;
use schedule::{Derived, Planned};

const MAX_EVENT_BYTES: usize = 16 * 1024;
const CHUNK_BYTES: usize = 4096;
const MAX_ACCOUNTS: usize = 64;
const SCHEMA: &str = "iroha.sumeragi_v2.multilane_scaling.collector_journal.v1";
const ENCODING: &str = "norito.canonical.signed_transaction.v1";
const WORKLOAD: &str = "self_owned_account_metadata_insert_v1";
const SELECTION: &str = "(zero_based_cohort_sequence + first8le(sha256(gscale-account-offset-v1:seed))) modulo pool_length";

/// Caller-selected execution geometry; no journal field supplies routing authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::kura::scaling_evidence::export) enum JournalVariant {
    OneLane,
    FourLane,
}
impl JournalVariant {
    fn text(self) -> &'static str {
        match self {
            Self::OneLane => "one_lane",
            Self::FourLane => "four_lane",
        }
    }
}

/// One independently selected, ordered universal account and exact expected route.
pub(in crate::kura::scaling_evidence::export) struct JournalAccount {
    pub(in crate::kura::scaling_evidence::export) authority: AccountId,
    pub(in crate::kura::scaling_evidence::export) route: RoutingDecision,
}

/// Exact rational offer rate and integer phase geometry supplied before collection.
pub(in crate::kura::scaling_evidence::export) struct JournalTiming {
    pub(in crate::kura::scaling_evidence::export) rate_numerator: u128,
    pub(in crate::kura::scaling_evidence::export) rate_denominator: u128,
    pub(in crate::kura::scaling_evidence::export) warmup_ns: i64,
    pub(in crate::kura::scaling_evidence::export) measurement_ns: i64,
    pub(in crate::kura::scaling_evidence::export) drain_ns: i64,
    pub(in crate::kura::scaling_evidence::export) submission_lag_bound_ns: i64,
}

/// Original declared collector bounds, compared exactly to the journal plan.
pub(in crate::kura::scaling_evidence::export) struct JournalBounds {
    pub(in crate::kura::scaling_evidence::export) preparation_lookahead: usize,
    pub(in crate::kura::scaling_evidence::export) preparation_concurrency: usize,
    pub(in crate::kura::scaling_evidence::export) preparation_ahead_ns: i64,
    pub(in crate::kura::scaling_evidence::export) max_submissions: usize,
    pub(in crate::kura::scaling_evidence::export) max_in_flight: usize,
    pub(in crate::kura::scaling_evidence::export) max_status_requests: usize,
    pub(in crate::kura::scaling_evidence::export) poll_interval_ns: i64,
    pub(in crate::kura::scaling_evidence::export) max_requests: usize,
}

/// Independent resource clock geometry; capture contents retain a separate verifier.
pub(in crate::kura::scaling_evidence::export) struct JournalSampling {
    pub(in crate::kura::scaling_evidence::export) interval_ns: i64,
    pub(in crate::kura::scaling_evidence::export) response_deadline_ns: i64,
    pub(in crate::kura::scaling_evidence::export) max_start_lag_ns: i64,
}

/// Complete independently owned input policy. This is never decoded from journal `plan`.
pub(in crate::kura::scaling_evidence::export) struct JournalExpectations {
    pub(in crate::kura::scaling_evidence::export) network_id: NetworkId,
    pub(in crate::kura::scaling_evidence::export) seed: String,
    pub(in crate::kura::scaling_evidence::export) pair_index: u8,
    pub(in crate::kura::scaling_evidence::export) variant: JournalVariant,
    pub(in crate::kura::scaling_evidence::export) accounts: Vec<JournalAccount>,
    pub(in crate::kura::scaling_evidence::export) timing: JournalTiming,
    pub(in crate::kura::scaling_evidence::export) bounds: JournalBounds,
    pub(in crate::kura::scaling_evidence::export) sampling: JournalSampling,
}

/// Collector observations only; these fields are not canonical state proof.
pub(in crate::kura::scaling_evidence::export) struct JournalObservation {
    pub(in crate::kura::scaling_evidence::export) sequence: usize,
    #[allow(dead_code, reason = "TODO: expose retained collector diagnostics")]
    pub(in crate::kura::scaling_evidence::export) scheduled_offset_ns: i64,
    pub(in crate::kura::scaling_evidence::export) offer_offset_ns: i64,
    pub(in crate::kura::scaling_evidence::export) acknowledgment_offset_ns: i64,
    pub(in crate::kura::scaling_evidence::export) applied_offset_ns: i64,
    pub(in crate::kura::scaling_evidence::export) block_height: u64,
    #[allow(dead_code, reason = "TODO: expose retained collector diagnostics")]
    pub(in crate::kura::scaling_evidence::export) status_attempts: usize,
}

/// Raw identity of the entire original byte stream, not a filesystem retention receipt.
pub(in crate::kura::scaling_evidence::export) struct JournalIdentity {
    #[allow(dead_code, reason = "TODO: bind journal identity to exported receipt")]
    pub(in crate::kura::scaling_evidence::export) raw_sha256: [u8; 32],
    #[allow(dead_code, reason = "TODO: bind journal identity to exported receipt")]
    pub(in crate::kura::scaling_evidence::export) byte_length: u64,
}

/// Opaque, non-cloneable complete read; there is no prefix or retry accessor.
pub(in crate::kura::scaling_evidence::export) struct CompleteJournal {
    scheduled: Vec<ScheduledRequest>,
    observations: Vec<JournalObservation>,
    identity: JournalIdentity,
}
impl CompleteJournal {
    /// Consume the complete read in original schedule order. No proof authority is returned.
    pub(in crate::kura::scaling_evidence::export) fn into_parts(
        self,
    ) -> (
        Vec<ScheduledRequest>,
        Vec<JournalObservation>,
        JournalIdentity,
    ) {
        (self.scheduled, self.observations, self.identity)
    }
}

struct Record {
    plan: Planned,
    bytes: Option<Vec<u8>>,
    hash: Option<String>,
    prepared: Option<i64>,
    offered: Option<i64>,
    accepted: Option<i64>,
    applied: Option<(i64, u64)>,
    local_applied: Option<(i64, u64)>,
    last_local_status: Option<i64>,
    local_attempts: usize,
    last_status: Option<i64>,
    attempts: usize,
    final_row: Option<JournalObservation>,
}
struct Active {
    index: usize,
    hash: String,
    digest: [u8; 32],
    length: usize,
    chunks: usize,
    next_chunk: usize,
    bytes: Vec<u8>,
}

struct Reader {
    expected: JournalExpectations,
    derived: Derived,
    records: Vec<Record>,
    active: Option<Active>,
    seen_hashes: BTreeSet<String>,
    canonical_limit: usize,
    canonical_bytes: usize,
    planned: bool,
    scheduled: usize,
    preflight: Vec<[u8; 32]>,
    clock: bool,
    post_started: bool,
    postconditions: usize,
    finals: usize,
    finished: bool,
    resources: events::ResourceState,
}

/// Reject invalid independent work geometry using the original schedule owner.
///
/// This performs no file read, journal parsing, staging or authority construction.
pub(in crate::kura::scaling_evidence::export) fn admit_expectations(
    expected: &JournalExpectations,
) -> Result<()> {
    schedule::derive(expected).map(|_| ())
}

/// Consume the complete independently pinned original JSONL stream in one operation.
///
/// Raw admission and SHA-256 precede the first JSON parse. Every physical event enters one
/// state machine. Any error or unwind drops the private owner and all retained requests.
pub(in crate::kura::scaling_evidence::export) fn read_original_journal(
    bytes: &[u8],
    expected_raw_sha256: [u8; 32],
    max_bytes: u64,
    expected: JournalExpectations,
) -> Result<CompleteJournal> {
    ensure!(
        (1..=MAX_PROOF_BYTES).contains(&max_bytes)
            && !bytes.is_empty()
            && u64::try_from(bytes.len())? <= max_bytes,
        "invalid original journal byte admission"
    );
    ensure!(
        iroha_crypto::sha256(bytes) == expected_raw_sha256,
        "original journal digest mismatch"
    );
    ensure!(
        bytes.last() == Some(&b'\n'),
        "journal requires final newline"
    );
    let derived = schedule::derive(&expected)?;
    // Even metadata admission is bounded by the actual retained stream: every planned row has
    // substantially more than 128 source bytes. Reject absurd counts before allocating slots.
    ensure!(
        derived.total <= bytes.len() / 128,
        "journal cannot contain declared schedule"
    );
    let mut records = Vec::new();
    records.try_reserve_exact(derived.total)?;
    for index in 0..derived.total {
        records.push(Record {
            plan: derived.plan(&expected, index)?,
            bytes: None,
            hash: None,
            prepared: None,
            offered: None,
            accepted: None,
            applied: None,
            local_applied: None,
            last_local_status: None,
            local_attempts: 0,
            last_status: None,
            attempts: 0,
            final_row: None,
        });
    }
    let mut reader = Reader {
        expected,
        derived,
        records,
        active: None,
        seen_hashes: BTreeSet::new(),
        canonical_limit: usize::try_from(max_bytes / 2)?,
        canonical_bytes: 0,
        planned: false,
        scheduled: 0,
        preflight: Vec::new(),
        clock: false,
        post_started: false,
        postconditions: 0,
        finals: 0,
        finished: false,
        resources: events::ResourceState::new(),
    };
    for line in bytes[..bytes.len() - 1].split(|byte| *byte == b'\n') {
        ensure!(!reader.finished, "physical event after collection finish");
        reader.consume(parse_event(line)?)?;
    }
    reader.finish(JournalIdentity {
        raw_sha256: expected_raw_sha256,
        byte_length: u64::try_from(bytes.len())?,
    })
}

fn parse_event(bytes: &[u8]) -> Result<Value> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_EVENT_BYTES,
        "journal event byte bound"
    );
    // A fixed 16 KiB source, at most 1024 values/entries and 64 nesting levels bounds the
    // owned Value tree. Hex chunks need 8192 decoded string bytes; no unbounded JSON DTO exists.
    json::preflight_slice(
        bytes,
        json::JsonPreflightLimits::new(
            MAX_EVENT_BYTES,
            1024,
            MAX_EVENT_BYTES,
            MAX_EVENT_BYTES,
            MAX_EVENT_BYTES,
            256,
            1024,
            1024,
            1024,
            64,
        ),
    )?;
    integer_tokens(bytes)?;
    // Preflight is lexical; the native owned decoder is also required for duplicate-key rejection.
    let row: Value = json::from_slice(bytes)?;
    numeric_values(&row)?;
    ensure!(row.as_object().is_some(), "journal event must be an object");
    Ok(row)
}
fn integer_tokens(bytes: &[u8]) -> Result<()> {
    // Norito preflight already established complete JSON grammar. This allocation-free policy
    // pass only limits numeric token work and rejects non-integer syntax before owned parsing.
    let mut cursor = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'"' => {
                cursor += 1;
                while cursor < bytes.len() {
                    match bytes[cursor] {
                        b'\\' => cursor += 2,
                        b'"' => {
                            cursor += 1;
                            break;
                        }
                        _ => cursor += 1,
                    }
                }
            }
            b'-' | b'0'..=b'9' => {
                let start = cursor;
                while cursor < bytes.len()
                    && !matches!(
                        bytes[cursor],
                        b' ' | b'\n' | b'\r' | b'\t' | b',' | b']' | b'}'
                    )
                {
                    cursor += 1;
                }
                let token = &bytes[start..cursor];
                ensure!(
                    token.len() <= 128
                        && token
                            .iter()
                            .enumerate()
                            .all(|(i, byte)| byte.is_ascii_digit() || (i == 0 && *byte == b'-')),
                    "journal numeric token must be a bounded integer"
                );
            }
            _ => cursor += 1,
        }
    }
    Ok(())
}
fn numeric_values(value: &Value) -> Result<()> {
    match value {
        Value::Number(number) => ensure!(
            number.as_i64().is_some() || number.as_u64().is_some(),
            "journal requires bounded integer tokens"
        ),
        Value::Array(values) => {
            for value in values {
                numeric_values(value)?;
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                numeric_values(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn fields(value: &Value, names: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("journal object required"))?;
    ensure!(
        object.len() == names.len() && names.iter().all(|name| object.contains_key(*name)),
        "journal object fields differ"
    );
    Ok(())
}
fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value> {
    value
        .get(name)
        .ok_or_else(|| eyre!("journal field missing"))
}
fn text<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    field(value, name)?
        .as_str()
        .ok_or_else(|| eyre!("journal text required"))
}
fn uint(value: &Value, name: &str, min: u64, max: u64) -> Result<u64> {
    let n = field(value, name)?
        .as_u64()
        .ok_or_else(|| eyre!("journal unsigned integer required"))?;
    ensure!((min..=max).contains(&n), "journal integer outside bound");
    Ok(n)
}
fn integer(value: &Value, name: &str) -> Result<i64> {
    field(value, name)?
        .as_i64()
        .ok_or_else(|| eyre!("journal signed integer required"))
}
fn equal_int(value: &Value, name: &str, expected: i64) -> Result<()> {
    ensure!(
        integer(value, name)? == expected,
        "journal integer disagrees with independent owner"
    );
    Ok(())
}
fn equal_uint(value: &Value, name: &str, expected: usize) -> Result<()> {
    ensure!(
        uint(value, name, 0, u64::MAX)? == u64::try_from(expected)?,
        "journal count disagrees with owner"
    );
    Ok(())
}
fn equal_text(value: &Value, name: &str, expected: &str) -> Result<()> {
    ensure!(
        text(value, name)? == expected,
        "journal text disagrees with owner"
    );
    Ok(())
}
fn digest(value: &str) -> Result<[u8; 32]> {
    ensure!(
        value.len() == 64 && lower_hex(value.as_bytes()),
        "journal digest must be lowercase SHA-256"
    );
    let mut bytes = [0; 32];
    hex::decode_to_slice(value, &mut bytes)?;
    Ok(bytes)
}
fn lower_hex(value: &[u8]) -> bool {
    value
        .iter()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

impl Reader {
    fn consume(&mut self, row: Value) -> Result<()> {
        let event = text(&row, "event")?;
        if let Some(active) = &self.active {
            let next = if active.next_chunk == active.chunks {
                "signed_request_retained"
            } else {
                "signed_request_chunk"
            };
            ensure!(
                event == next,
                "physical event interleaves signed request retention"
            );
        }
        ensure!(
            self.planned || event == "plan",
            "first physical event must be plan"
        );
        match event {
            "signed_request_begin" => self.begin(&row),
            "signed_request_chunk" => self.chunk(&row),
            "signed_request_retained" => self.retained(&row),
            "prepared" | "offer" | "accepted" => self.progress(&row, event),
            "status" | "status_missing" | "local_status" | "local_status_missing" => {
                self.status(&row, event)
            }
            _ => self.consume_control(&row, event),
        }
    }
    fn running(&self) -> Result<()> {
        ensure!(
            self.clock && !self.post_started && !self.finished,
            "transaction event outside collection"
        );
        Ok(())
    }
    fn index(&self, row: &Value) -> Result<usize> {
        Ok(usize::try_from(uint(
            row,
            "index",
            0,
            u64::try_from(self.records.len() - 1)?,
        )?)?)
    }
    fn begin(&mut self, row: &Value) -> Result<()> {
        self.running()?;
        fields(
            row,
            &[
                "event",
                "index",
                "plan",
                "hash",
                "encoding",
                "byte_length",
                "canonical_sha256",
                "chunk_count",
            ],
        )?;
        ensure!(self.active.is_none(), "duplicate signed begin");
        let index = self.index(row)?;
        if index >= self.derived.warmup {
            ensure!(
                self.records[..self.derived.warmup]
                    .iter()
                    .all(|r| r.accepted.is_some() && r.applied.is_some()),
                "measurement retention before complete warmup drain"
            );
        }
        ensure!(
            self.records[index].bytes.is_none(),
            "duplicate signed request index"
        );
        self.records[index].plan.matches(field(row, "plan")?)?;
        equal_text(row, "encoding", ENCODING)?;
        let hash = text(row, "hash")?;
        digest(hash)?;
        ensure!(
            !self.seen_hashes.contains(hash),
            "duplicate signed request hash"
        );
        let length = usize::try_from(uint(row, "byte_length", 1, MAX_TRANSACTION_BYTES as u64)?)?;
        let chunks = usize::try_from(uint(
            row,
            "chunk_count",
            1,
            (MAX_TRANSACTION_BYTES / CHUNK_BYTES) as u64,
        )?)?;
        ensure!(
            chunks == length.div_ceil(CHUNK_BYTES),
            "signed chunk geometry mismatch"
        );
        ensure!(
            length <= self.canonical_limit - self.canonical_bytes,
            "signed request cumulative byte admission"
        );
        let canonical_digest = digest(text(row, "canonical_sha256")?)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length)?;
        self.active = Some(Active {
            index,
            hash: hash.to_owned(),
            digest: canonical_digest,
            length,
            chunks,
            next_chunk: 0,
            bytes,
        });
        Ok(())
    }
    fn chunk(&mut self, row: &Value) -> Result<()> {
        fields(
            row,
            &["event", "index", "chunk_index", "offset", "bytes_hex"],
        )?;
        let active = self
            .active
            .as_mut()
            .ok_or_else(|| eyre!("signed chunk without begin"))?;
        equal_uint(row, "index", active.index)?;
        equal_uint(row, "chunk_index", active.next_chunk)?;
        equal_uint(row, "offset", active.bytes.len())?;
        let length = CHUNK_BYTES.min(active.length - active.bytes.len());
        let encoded = text(row, "bytes_hex")?;
        ensure!(
            length > 0 && encoded.len() == 2 * length && lower_hex(encoded.as_bytes()),
            "signed chunk exact lowercase length required"
        );
        let start = active.bytes.len();
        active.bytes.resize(start + length, 0);
        hex::decode_to_slice(encoded, &mut active.bytes[start..])?;
        active.next_chunk += 1;
        Ok(())
    }
    fn retained(&mut self, row: &Value) -> Result<()> {
        fields(
            row,
            &[
                "event",
                "index",
                "hash",
                "byte_length",
                "canonical_sha256",
                "chunk_count",
            ],
        )?;
        let active = self
            .active
            .take()
            .ok_or_else(|| eyre!("signed retained without begin"))?;
        equal_uint(row, "index", active.index)?;
        equal_text(row, "hash", &active.hash)?;
        equal_uint(row, "byte_length", active.length)?;
        equal_uint(row, "chunk_count", active.chunks)?;
        ensure!(
            active.next_chunk == active.chunks
                && active.bytes.len() == active.length
                && digest(text(row, "canonical_sha256")?)? == active.digest
                && iroha_crypto::sha256(&active.bytes) == active.digest,
            "signed request raw bytes/digest mismatch"
        );
        let transaction: SignedTransaction = canonical(&active.bytes)?;
        transaction.verify_signature()?;
        let record = &mut self.records[active.index];
        let account = &self.expected.accounts[record.plan.account_index];
        ensure!(
            transaction.hash().to_string() == active.hash,
            "signed canonical transaction hash mismatch"
        );
        ensure!(
            transaction.network_id() == Some(&self.expected.network_id)
                && transaction.authority() == &account.authority,
            "signed transaction network or selected authority mismatch"
        );
        ensure!(
            transaction.instructions()
                == &expected_executable(&account.authority, &record.plan.logical_id)?,
            "signed transaction is not the exact scheduled useful effect"
        );
        self.canonical_bytes += active.length;
        self.seen_hashes.insert(active.hash.clone());
        record.hash = Some(active.hash);
        record.bytes = Some(active.bytes);
        Ok(())
    }
    fn progress(&mut self, row: &Value, event: &str) -> Result<()> {
        self.running()?;
        fields(row, &["event", "index", "hash", "offset_ns"])?;
        let index = self.index(row)?;
        let offset = integer(row, "offset_ns")?;
        let record = &mut self.records[index];
        equal_text(
            row,
            "hash",
            record
                .hash
                .as_deref()
                .ok_or_else(|| eyre!("progress before retained request"))?,
        )?;
        ensure!(
            offset >= self.derived.origin,
            "progress before clock origin"
        );
        match event {
            "prepared" => {
                ensure!(
                    record.prepared.is_none() && record.offered.is_none(),
                    "duplicate or late prepared event"
                );
                ensure!(
                    record
                        .plan
                        .scheduled_offset_ns
                        .checked_sub(self.expected.bounds.preparation_ahead_ns)
                        .is_some_and(|earliest| offset >= earliest),
                    "prepared event precedes allowed lookahead time"
                );
                record.prepared = Some(offset);
            }
            "offer" => {
                ensure!(
                    record.offered.is_none() && record.prepared.is_some_and(|v| v <= offset),
                    "offer before preparation or duplicate offer"
                );
                let lag = offset
                    .checked_sub(record.plan.scheduled_offset_ns)
                    .ok_or_else(|| eyre!("offer offset overflow"))?;
                ensure!(
                    (0..=self.expected.timing.submission_lag_bound_ns).contains(&lag)
                        && offset < self.derived.end(&self.expected, record.plan.phase),
                    "offer exceeds independent schedule"
                );
                record.offered = Some(offset);
            }
            "accepted" => {
                ensure!(
                    record.accepted.is_none()
                        && record.offered.is_some_and(|v| v <= offset)
                        && self
                            .derived
                            .within(&self.expected, record.plan.phase, offset),
                    "accepted event outside exact offer/deadline"
                );
                record.accepted = Some(offset);
            }
            _ => unreachable!(),
        }
        Ok(())
    }
    fn status(&mut self, row: &Value, event: &str) -> Result<()> {
        self.running()?;
        let local = matches!(event, "local_status" | "local_status_missing");
        let present = matches!(event, "status" | "local_status");
        let present_fields = [
            "event",
            "index",
            "offset_ns",
            "expected_hash",
            "hash_matches",
            if local {
                "local_scope_matches"
            } else {
                "global_scope_matches"
            },
            "resolved_from",
            "status",
            "block_height",
        ];
        fields(
            row,
            if present {
                &present_fields
            } else {
                &["event", "index", "offset_ns", "hash"]
            },
        )?;
        let index = self.index(row)?;
        let offset = integer(row, "offset_ns")?;
        let record = &mut self.records[index];
        let hash = record
            .hash
            .as_deref()
            .ok_or_else(|| eyre!("status before retention"))?;
        equal_text(row, if present { "expected_hash" } else { "hash" }, hash)?;
        let (applied, last_status, attempts) = if local {
            (
                &mut record.local_applied,
                &mut record.last_local_status,
                &mut record.local_attempts,
            )
        } else {
            (
                &mut record.applied,
                &mut record.last_status,
                &mut record.attempts,
            )
        };
        ensure!(
            record.offered.is_some_and(|v| v <= offset) && applied.is_none(),
            "status before offer or after terminal Applied"
        );
        if let Some(previous) = *last_status {
            ensure!(
                previous
                    .checked_add(self.expected.bounds.poll_interval_ns)
                    .is_some_and(|v| v <= offset),
                "status poll interval shortened"
            );
        }
        *last_status = Some(offset);
        *attempts = attempts
            .checked_add(1)
            .ok_or_else(|| eyre!("status count overflow"))?;
        if !present {
            return Ok(());
        }
        ensure!(
            field(row, "hash_matches")?.as_bool() == Some(true)
                && field(
                    row,
                    if local {
                        "local_scope_matches"
                    } else {
                        "global_scope_matches"
                    }
                )?
                .as_bool()
                    == Some(true),
            if local {
                "status hash/local scope mismatch"
            } else {
                "status hash/global scope mismatch"
            }
        );
        let status = text(row, "status")?;
        let source = text(row, "resolved_from")?;
        let height = if field(row, "block_height")?.is_null() {
            None
        } else {
            Some(uint(row, "block_height", 0, u64::MAX)?)
        };
        match (status, source) {
            ("Applied", "state") => {
                let height = height
                    .filter(|v| *v > 0)
                    .ok_or_else(|| eyre!("StateApplied height must be positive"))?;
                ensure!(
                    record.offered.is_some_and(|v| v < offset)
                        && self
                            .derived
                            .within(&self.expected, record.plan.phase, offset),
                    "StateApplied outside exact offer/deadline"
                );
                *applied = Some((offset, height));
                if let (Some((_, global_height)), Some((_, local_height))) =
                    (record.applied, record.local_applied)
                {
                    ensure!(
                        global_height == local_height,
                        "global and local StateApplied heights differ"
                    );
                }
            }
            ("Queued" | "Approved" | "Committed", "state" | "cache" | "queue")
            | ("Applied" | "Rejected" | "Expired", "cache" | "queue") => {}
            _ => return Err(eyre!("journal status is failed or unknown")),
        }
        Ok(())
    }
    fn finish(self, identity: JournalIdentity) -> Result<CompleteJournal> {
        ensure!(
            self.finished && self.active.is_none() && self.finals == self.records.len(),
            "incomplete original journal"
        );
        self.observable_occupancy()?;
        let mut scheduled = Vec::new();
        let mut observations = Vec::new();
        scheduled.try_reserve_exact(self.records.len())?;
        observations.try_reserve_exact(self.records.len())?;
        for record in self.records {
            scheduled.push(ScheduledRequest {
                logical_id: record.plan.logical_id,
                phase: record.plan.phase,
                signed_transaction: record
                    .bytes
                    .ok_or_else(|| eyre!("missing original signed bytes"))?,
                route: self.expected.accounts[record.plan.account_index].route,
            });
            observations.push(
                record
                    .final_row
                    .ok_or_else(|| eyre!("missing complete final row"))?,
            );
        }
        Ok(CompleteJournal {
            scheduled,
            observations,
            identity,
        })
    }

    fn observable_occupancy(&self) -> Result<()> {
        // These are conservative lower bounds, not a reconstruction of runtime tasks. An
        // actual submission can start before its offer and remain occupied after its captured
        // response. A request remains unsettled until acknowledgment and both global/local Applied.
        // Strictly overlapping observed intervals above the declared caps are impossible even
        // under those unknown processing delays. Preparation/status start events do not exist,
        // so this reader makes no claim to authenticate their full runtime concurrency.
        let capacity = self
            .records
            .len()
            .checked_mul(2)
            .ok_or_else(|| eyre!("occupancy endpoint count overflow"))?;
        let mut endpoints = Vec::new();
        endpoints.try_reserve_exact(capacity)?;
        for (unsettled, maximum, message) in [
            (
                false,
                self.expected.bounds.max_submissions,
                "journal observable submission occupancy exceeds declared bound",
            ),
            (
                true,
                self.expected.bounds.max_in_flight,
                "journal observable in-flight occupancy exceeds declared bound",
            ),
        ] {
            endpoints.clear();
            for record in &self.records {
                let final_row = record
                    .final_row
                    .as_ref()
                    .ok_or_else(|| eyre!("occupancy requires every joined final row"))?;
                let start = final_row.offer_offset_ns;
                let end = if unsettled {
                    final_row
                        .acknowledgment_offset_ns
                        .max(final_row.applied_offset_ns)
                        .max(
                            record
                                .local_applied
                                .ok_or_else(|| eyre!("occupancy requires local StateApplied"))?
                                .0,
                        )
                } else {
                    final_row.acknowledgment_offset_ns
                };
                ensure!(end >= start, "occupancy interval is reversed");
                if start == end {
                    continue;
                }
                endpoints.push((start, true));
                endpoints.push((end, false));
            }
            // Ends sort before starts at an equal timestamp. Half-open intervals do not prove
            // overlap at a zero-width boundary, even if runtime task cleanup was delayed.
            endpoints.sort_unstable();
            let mut active = 0_usize;
            for (_, start) in &endpoints {
                if *start {
                    active = active
                        .checked_add(1)
                        .ok_or_else(|| eyre!("occupancy count overflow"))?;
                    ensure!(active <= maximum, message);
                } else {
                    active = active
                        .checked_sub(1)
                        .ok_or_else(|| eyre!("occupancy endpoint underflow"))?;
                }
            }
            ensure!(active == 0, "occupancy endpoint census incomplete");
        }
        Ok(())
    }
}

#[cfg(test)]
pub(in crate::kura::scaling_evidence::export) mod tests;

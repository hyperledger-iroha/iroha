//! Bounded persistent open-loop transaction collection for the first-release scaling gate.
//!
//! This command writes account metadata and verifies every planned effect; deployment, routing,
//! and the five-pair statistical gate retain their existing owners. Failed collection never
//! publishes a passing trace or replays an ambiguous submission.

use iroha_model_base::name::Name;
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet, BinaryHeap},
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    time::Duration,
};

use eyre::{Result, WrapErr, bail, eyre};
use futures::{FutureExt, StreamExt, future::BoxFuture, stream::FuturesUnordered};
use iroha::{
    client::{
        AccountClient, AccountTransactionDraft, Client, FeeQuoteRequest, PreparedTransactionPayload,
    },
    config::Config,
    data_model::prelude::*,
};
use iroha_crypto::HashOf;
use iroha_torii_shared::PipelineTransactionStatusResponse;
use norito::json::{self, Value};
use sha2::{Digest, Sha256};
use tokio::time::Instant;

use crate::{Run, RunContext};

mod resource;
mod workload;

const NS: i64 = 1_000_000_000;
const MAX_ROWS: usize = 1_000_000;
const MAX_FILE_BYTES: usize = 256 * 1024 * 1024;
const MAX_EVENT_BYTES: usize = 16 * 1024;
const MAX_ACCOUNTS: usize = 64;
const TRACE_SCHEMA: &str = "iroha.sumeragi_v2.multilane_scaling.trace.v1";
const LOGICAL_DERIVATION: &str = "sha256(seed + ':' + cohort + ':' + decimal_sequence)";

type TransactionHash = HashOf<SignedTransaction>;
type Work<T> = FuturesUnordered<BoxFuture<'static, (usize, T)>>;

/// Collect a complete scheduled transaction cohort with persistent SDK clients.
#[derive(clap::Args, Debug)]
pub struct Args {
    /// Required resource capture process and fixed sampling geometry.
    #[command(flatten)]
    resource: resource::Args,
    /// Pair index in the fixed five-pair experiment.
    #[arg(long)]
    pair_index: u8,
    /// Declared execution-lane variant; this command does not activate lanes.
    #[arg(long, value_enum)]
    variant: Variant,
    /// Exact lowercase SHA-256 pair seed supplied by the scaling runner.
    #[arg(long)]
    seed: String,
    /// Fixed decimal offers per second, independent of responses.
    #[arg(long)]
    offered_load_tps: String,
    /// Fixed decimal warmup duration, excluded from measurement.
    #[arg(long)]
    warmup_seconds: String,
    /// Fixed decimal measurement duration.
    #[arg(long)]
    measurement_seconds: String,
    /// Fixed warmup and postmeasurement drain duration, at most 300 seconds.
    #[arg(long)]
    drain_seconds: String,
    /// Maximum actual submission lag, at most one quarter of an arrival period.
    #[arg(long)]
    max_submission_lag_ms: String,
    /// Existing funded canonical accounts in fixed order; use a multiple of four, at most 64.
    /// Each nonzero cohort must contain complete pool rounds, with at most 1024 total writes per account.
    #[arg(long = "account-config", value_name = "PATH")]
    account_configs: Vec<PathBuf>,
    /// New strict trace path, published only after successful collection.
    #[arg(long, value_name = "PATH")]
    trace_out: PathBuf,
    /// New bounded diagnostic JSON-lines journal, retained on failure.
    #[arg(long, value_name = "PATH")]
    diagnostic_out: PathBuf,
    /// Maximum prepared or preparing requests ahead of the next scheduled offer.
    #[arg(long, default_value_t = 256)]
    preparation_lookahead: usize,
    /// Maximum concurrent quote/sign preparations.
    #[arg(long, default_value_t = 8)]
    preparation_concurrency: usize,
    /// Maximum time before an offer at which preparation can start.
    #[arg(long, default_value_t = 1000)]
    preparation_ahead_ms: u64,
    /// Maximum concurrent submission requests; saturation invalidates the trial.
    #[arg(long, default_value_t = 256)]
    max_submissions: usize,
    /// Maximum offered requests awaiting acknowledgment or Applied.
    #[arg(long, default_value_t = 4096)]
    max_in_flight: usize,
    /// Maximum concurrent canonical global status reads.
    #[arg(long, default_value_t = 64)]
    max_status_requests: usize,
    /// Fixed delay between completed nonterminal observations of one hash.
    #[arg(long, default_value_t = 50)]
    poll_interval_ms: u64,
    /// Maximum queued diagnostic events; saturation invalidates the trial.
    #[arg(long, default_value_t = 4096)]
    journal_capacity: usize,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum Variant {
    #[value(name = "one_lane")]
    OneLane,
    #[value(name = "four_lane")]
    FourLane,
}
impl Variant {
    fn text(self) -> &'static str {
        match self {
            Self::OneLane => "one_lane",
            Self::FourLane => "four_lane",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cohort {
    Warmup,
    Measurement,
}
impl Cohort {
    fn text(self) -> &'static str {
        match self {
            Self::Warmup => "warmup",
            Self::Measurement => "measurement",
        }
    }
}

#[derive(Clone, Debug)]
struct Planned {
    cohort: Cohort,
    sequence: usize,
    logical_id: String,
    scheduled_offset_ns: i64,
    account_index: usize,
}
impl Planned {
    fn value(&self) -> Value {
        norito::json!({"cohort": (self.cohort.text()), "sequence": (self.sequence),
            "logical_id": (self.logical_id), "scheduled_offset_ns": (self.scheduled_offset_ns),
            "account_index": (self.account_index)})
    }
}

#[derive(Debug)]
struct Record {
    plan: Planned,
    hash: Option<TransactionHash>,
    offer_ns: Option<i64>,
    acknowledgment_ns: Option<i64>,
    submission_finished: bool,
    applied: Option<(i64, u64)>,
    failure: Option<String>,
    attempts: usize,
}
impl Record {
    fn new(plan: Planned) -> Self {
        Self {
            plan,
            hash: None,
            offer_ns: None,
            acknowledgment_ns: None,
            submission_finished: false,
            applied: None,
            failure: None,
            attempts: 0,
        }
    }
    fn settled(&self) -> bool {
        self.submission_finished && self.applied.is_some()
    }
    fn trace_value(&self) -> Result<Value> {
        if self.failure.is_some() {
            bail!("failed request cannot become a strict trace row");
        }
        let hash = self
            .hash
            .ok_or_else(|| eyre!("scheduled request has no prepared hash"))?
            .to_string();
        let offer = self
            .offer_ns
            .ok_or_else(|| eyre!("scheduled request was never offered"))?;
        let ack = self
            .acknowledgment_ns
            .ok_or_else(|| eyre!("offered request has no admission acknowledgment"))?;
        let (applied, height) = self
            .applied
            .ok_or_else(|| eyre!("accepted request has no StateApplied"))?;
        Ok(
            norito::json!({"cohort": (self.plan.cohort.text()), "sequence": (self.plan.sequence),
            "logical_id": (self.plan.logical_id), "hash": hash,
            "scheduled_offset_ns": (self.plan.scheduled_offset_ns), "offer_offset_ns": offer,
            "submission_lag_ns": (offer - self.plan.scheduled_offset_ns),
            "acknowledgment": {"offset_ns": ack, "hash": hash, "status": "Accepted", "rejection": null},
            "applied": {"offset_ns": applied, "hash": hash, "scope": "global", "resolved_from": "state",
                "status": "Applied", "block_height": height}}),
        )
    }
    fn diagnostic_value(&self) -> Value {
        norito::json!({"event": "request_final", "plan": (self.plan.value()),
            "hash": (self.hash.map(|hash| hash.to_string())), "offer_offset_ns": (self.offer_ns),
            "acknowledgment_offset_ns": (self.acknowledgment_ns),
            "applied_offset_ns": (self.applied.map(|value| value.0)),
            "block_height": (self.applied.map(|value| value.1)), "status_attempts": (self.attempts),
            "submission_finished": (self.submission_finished), "failure": (self.failure)})
    }
}

#[derive(Clone, Debug)]
struct Schedule {
    rate_numerator: u128,
    rate_denominator: u128,
    warmup_ns: i64,
    measurement_ns: i64,
    drain_ns: i64,
    lag_ns: i64,
}

/// Parse an exact nonnegative decimal without binary floating-point rounding.
fn decimal(value: &str) -> Result<(u128, u128)> {
    if value.is_empty() || value.len() > 48 || value.trim() != value {
        bail!("invalid bounded decimal");
    }
    let (base, exponent) = if let Some(position) = value.find(['e', 'E']) {
        (
            &value[..position],
            value[position + 1..]
                .parse::<i32>()
                .wrap_err("invalid decimal exponent")?,
        )
    } else {
        (value, 0)
    };
    if !(-18..=18).contains(&exponent) {
        bail!("decimal exponent exceeds bound");
    }
    let mut digits = String::new();
    let mut fraction = None;
    for ch in base.chars() {
        if ch == '.' && fraction.is_none() {
            fraction = Some(0_i32);
        } else if ch.is_ascii_digit() {
            digits.push(ch);
            if let Some(count) = &mut fraction {
                *count += 1;
            }
        } else {
            bail!("decimal must contain only unsigned base-ten digits");
        }
    }
    if digits.is_empty() {
        bail!("decimal has no digits");
    }
    let mut numerator = digits
        .parse::<u128>()
        .wrap_err("decimal exceeds integer bound")?;
    let scale = fraction.unwrap_or(0) - exponent;
    if scale.abs() > 18 {
        bail!("decimal precision exceeds 18 places");
    }
    let denominator = if scale >= 0 {
        10_u128.pow(scale as u32)
    } else {
        numerator = numerator
            .checked_mul(10_u128.pow((-scale) as u32))
            .ok_or_else(|| eyre!("decimal overflow"))?;
        1
    };
    Ok((numerator, denominator))
}
fn decimal_ns(value: &str, units: u128) -> Result<i64> {
    let (numerator, denominator) = decimal(value)?;
    let scaled = numerator
        .checked_mul(units)
        .ok_or_else(|| eyre!("time overflow"))?;
    if scaled % denominator != 0 {
        bail!("time must represent exact integer nanoseconds");
    }
    i64::try_from(scaled / denominator).wrap_err("time exceeds signed nanosecond bound")
}
impl Schedule {
    fn from_args(args: &Args) -> Result<Self> {
        let (rate_numerator, rate_denominator) = decimal(&args.offered_load_tps)?;
        if rate_numerator == 0 {
            bail!("offered load must be positive");
        }
        let schedule = Self {
            rate_numerator,
            rate_denominator,
            warmup_ns: decimal_ns(&args.warmup_seconds, NS as u128)?,
            measurement_ns: decimal_ns(&args.measurement_seconds, NS as u128)?,
            drain_ns: decimal_ns(&args.drain_seconds, NS as u128)?,
            lag_ns: decimal_ns(&args.max_submission_lag_ms, 1_000_000)?,
        };
        if schedule.measurement_ns == 0 || !(1..=300 * NS).contains(&schedule.drain_ns) {
            bail!("measurement must be positive and drain must be in (0, 300] seconds");
        }
        let period_numerator = (NS as u128)
            .checked_mul(rate_denominator)
            .ok_or_else(|| eyre!("rate overflow"))?;
        if rate_numerator > period_numerator
            || (schedule.lag_ns as u128)
                .checked_mul(4)
                .and_then(|v| v.checked_mul(rate_numerator))
                .is_none_or(|v| v > period_numerator)
        {
            bail!("submission lag exceeds one quarter of an arrival period");
        }
        schedule
            .warmup_ns
            .checked_add(schedule.measurement_ns)
            .and_then(|v| v.checked_add(schedule.drain_ns * 2))
            .ok_or_else(|| eyre!("phase bounds overflow"))?;
        if schedule
            .count(Cohort::Warmup)?
            .checked_add(schedule.count(Cohort::Measurement)?)
            .is_none_or(|count| count > MAX_ROWS)
        {
            bail!("schedule exceeds one million requests");
        }
        Ok(schedule)
    }
    fn count(&self, cohort: Cohort) -> Result<usize> {
        let duration = match cohort {
            Cohort::Warmup => self.warmup_ns,
            Cohort::Measurement => self.measurement_ns,
        };
        let numerator = (duration as u128)
            .checked_mul(self.rate_numerator)
            .ok_or_else(|| eyre!("schedule count overflow"))?;
        let denominator = (NS as u128)
            .checked_mul(self.rate_denominator)
            .ok_or_else(|| eyre!("schedule count overflow"))?;
        usize::try_from(numerator.div_ceil(denominator)).wrap_err("schedule count overflow")
    }
    fn start(&self, cohort: Cohort) -> i64 {
        match cohort {
            Cohort::Warmup => -self.warmup_ns - self.drain_ns,
            Cohort::Measurement => 0,
        }
    }
    fn end(&self, cohort: Cohort) -> i64 {
        match cohort {
            Cohort::Warmup => -self.drain_ns,
            Cohort::Measurement => self.measurement_ns,
        }
    }
    fn deadline(&self, cohort: Cohort) -> i64 {
        match cohort {
            Cohort::Warmup => 0,
            Cohort::Measurement => self.measurement_ns + self.drain_ns,
        }
    }
    fn within_deadline(&self, cohort: Cohort, offset: i64) -> bool {
        offset < self.deadline(cohort)
            || (cohort == Cohort::Measurement && offset == self.deadline(cohort))
    }
    fn plan(&self, seed: &str, account_count: usize) -> Result<Vec<Record>> {
        if account_count == 0 {
            bail!("account pool is empty");
        }
        let account_offset = workload::account_offset(seed, account_count)?;
        let mut records = Vec::new();
        records.try_reserve(self.count(Cohort::Warmup)? + self.count(Cohort::Measurement)?)?;
        for cohort in [Cohort::Warmup, Cohort::Measurement] {
            for index in 0..self.count(cohort)? {
                let sequence = index + 1;
                let logical =
                    Sha256::digest(format!("{seed}:{}:{sequence}", cohort.text()).as_bytes());
                let offset = (index as u128)
                    .checked_mul(NS as u128)
                    .and_then(|v| v.checked_mul(self.rate_denominator))
                    .ok_or_else(|| eyre!("schedule offset overflow"))?
                    / self.rate_numerator;
                let scheduled_offset_ns = self
                    .start(cohort)
                    .checked_add(i64::try_from(offset)?)
                    .ok_or_else(|| eyre!("schedule offset overflow"))?;
                records.push(Record::new(Planned {
                    cohort,
                    sequence,
                    logical_id: hex::encode(logical),
                    scheduled_offset_ns,
                    account_index: (index % account_count + account_offset) % account_count,
                }));
            }
        }
        Ok(records)
    }
    fn offer(&self, plan: &Planned, now: i64) -> Result<()> {
        let lag = now
            .checked_sub(plan.scheduled_offset_ns)
            .ok_or_else(|| eyre!("offer offset overflow"))?;
        if lag < 0 || lag > self.lag_ns || now >= self.end(plan.cohort) {
            bail!("missed fixed offer schedule or submission-lag bound");
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct Bounds {
    lookahead: usize,
    preparations: usize,
    ahead_ns: i64,
    submissions: usize,
    in_flight: usize,
    observations: usize,
    poll_ns: i64,
}
impl Bounds {
    fn from_args(args: &Args) -> Result<Self> {
        for (name, value, maximum) in [
            ("preparation-lookahead", args.preparation_lookahead, 4096),
            ("preparation-concurrency", args.preparation_concurrency, 32),
            ("max-submissions", args.max_submissions, 4096),
            ("max-in-flight", args.max_in_flight, 16384),
            ("max-status-requests", args.max_status_requests, 256),
            ("journal-capacity", args.journal_capacity, 16384),
        ] {
            if value == 0 || value > maximum {
                bail!("{name} must be in 1..={maximum}");
            }
        }
        if !(1..=30_000).contains(&args.preparation_ahead_ms)
            || !(1..=10_000).contains(&args.poll_interval_ms)
        {
            bail!("preparation/poll duration exceeds its positive bound");
        }
        Ok(Self {
            lookahead: args.preparation_lookahead,
            preparations: args.preparation_concurrency,
            ahead_ns: args.preparation_ahead_ms as i64 * 1_000_000,
            submissions: args.max_submissions,
            in_flight: args.max_in_flight,
            observations: args.max_status_requests,
            poll_ns: args.poll_interval_ms as i64 * 1_000_000,
        })
    }
}

trait Clock: Send + Sync + 'static {
    fn now(&self) -> i64;
    fn sleep(self: Arc<Self>, offset: i64) -> BoxFuture<'static, i64>;
    fn timer_reached(&self, _offset: i64) {}
}
struct LiveClock {
    started: Instant,
    initial_offset: i64,
}
impl Clock for LiveClock {
    fn now(&self) -> i64 {
        self.initial_offset
            .saturating_add(i64::try_from(self.started.elapsed().as_nanos()).unwrap_or(i64::MAX))
    }
    fn sleep(self: Arc<Self>, offset: i64) -> BoxFuture<'static, i64> {
        async move {
            let delay = offset.saturating_sub(self.initial_offset).max(0) as u64;
            tokio::time::sleep_until(self.started + Duration::from_nanos(delay)).await;
            offset
        }
        .boxed()
    }
}

struct Prepared<P> {
    payload: P,
    hash: TransactionHash,
    account_index: usize,
}
trait Backend: Send + Sync + 'static {
    type Payload: Send + 'static;
    fn prepare(
        self: Arc<Self>,
        plan: Planned,
    ) -> BoxFuture<'static, Result<Prepared<Self::Payload>>>;
    fn submit(
        self: Arc<Self>,
        payload: Prepared<Self::Payload>,
    ) -> BoxFuture<'static, Result<TransactionHash>>;
    fn observe(
        self: Arc<Self>,
        account_index: usize,
        hash: TransactionHash,
    ) -> BoxFuture<'static, Result<Option<PipelineTransactionStatusResponse>>>;
}
struct SdkBackend {
    clients: Vec<Client>,
    accounts: Vec<AccountClient>,
    metadata: Metadata,
    fee: FeePaymentIntent,
}
impl Backend for SdkBackend {
    type Payload = PreparedTransactionPayload;
    fn prepare(
        self: Arc<Self>,
        plan: Planned,
    ) -> BoxFuture<'static, Result<Prepared<Self::Payload>>> {
        async move {
            let account = self.accounts[plan.account_index].clone();
            let mut metadata = self.metadata.clone();
            let logical_key = "gscale_logical_id".parse::<Name>()?;
            if metadata.contains(&logical_key) {
                bail!(
                    "transaction metadata reserves gscale_logical_id for exact workload identity"
                );
            }
            metadata.insert(logical_key, plan.logical_id.as_str());
            let executable = workload::executable(account.authority(), &plan)?;
            crate::validate_executable_fee_payment(&executable, &self.fee)?;
            let fee = self.fee.clone();
            let prepare_account = account.clone();
            let mut payload = tokio::task::spawn_blocking(move || {
                prepare_account
                    .prepare_transaction(AccountTransactionDraft::new(executable, fee, metadata))
            })
            .await??;
            let quote = account
                .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })
                .await?;
            crate::validate_executable_fee_payment(&payload.instructions, &quote.intent)?;
            payload.fee_payment = quote.intent;
            let encoded =
                tokio::task::spawn_blocking(move || -> Result<PreparedTransactionPayload> {
                    let transaction = account.sign_transaction(payload)?;
                    Ok(PreparedTransactionPayload::from_transaction(&transaction))
                })
                .await??;
            Ok(Prepared {
                hash: encoded.hash(),
                payload: encoded,
                account_index: plan.account_index,
            })
        }
        .boxed()
    }
    fn submit(
        self: Arc<Self>,
        payload: Prepared<Self::Payload>,
    ) -> BoxFuture<'static, Result<TransactionHash>> {
        async move {
            self.accounts[payload.account_index]
                .submit_prepared_transaction_payload(&payload.payload)
                .await
        }
        .boxed()
    }
    fn observe(
        self: Arc<Self>,
        account_index: usize,
        hash: TransactionHash,
    ) -> BoxFuture<'static, Result<Option<PipelineTransactionStatusResponse>>> {
        async move {
            self.clients[account_index]
                .fetch_transaction_status_response_global(hash)
                .await
        }
        .boxed()
    }
}

trait Recorder: Send + Sync + 'static {
    fn record(&self, event: Value) -> Result<()>;
}
enum JournalCommand {
    Record(Value),
    Barrier(mpsc::SyncSender<()>),
}
struct JournalSender(mpsc::SyncSender<JournalCommand>);
impl Recorder for JournalSender {
    fn record(&self, event: Value) -> Result<()> {
        self.0
            .try_send(JournalCommand::Record(event))
            .map_err(|_| eyre!("bounded diagnostic writer is saturated or unavailable"))
    }
}
struct Journal {
    sender: Arc<JournalSender>,
    worker: std::thread::JoinHandle<Result<()>>,
}
fn new_owned_file(path: &Path) -> Result<File> {
    let parent = path.parent().ok_or_else(|| eyre!("output has no parent"))?;
    if parent.canonicalize()? != parent {
        bail!("output requires an absolute canonical existing parent");
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .wrap_err("output must be a new regular file")?;
    File::open(parent)?.sync_all()?;
    Ok(file)
}
fn bounded_write(writer: &mut impl Write, written: &mut usize, bytes: &[u8]) -> Result<()> {
    let total = written
        .checked_add(bytes.len())
        .ok_or_else(|| eyre!("artifact byte count overflow"))?;
    if total > MAX_FILE_BYTES {
        bail!("collector artifact exceeds 256 MiB; truncation is forbidden");
    }
    writer.write_all(bytes)?;
    *written = total;
    Ok(())
}
impl Journal {
    fn start(path: &Path, capacity: usize) -> Result<Self> {
        let file = new_owned_file(path)?;
        let (sender, receiver) = mpsc::sync_channel::<JournalCommand>(capacity);
        let worker = std::thread::spawn(move || {
            let mut writer = BufWriter::new(file);
            let mut written = 0;
            for command in receiver {
                let event = match command {
                    JournalCommand::Record(event) => event,
                    JournalCommand::Barrier(acknowledgment) => {
                        writer.flush()?;
                        writer.get_ref().sync_all()?;
                        let _ = acknowledgment.send(());
                        continue;
                    }
                };
                let bytes = json::to_vec(&event)?;
                if bytes.len() > MAX_EVENT_BYTES {
                    bail!("diagnostic event exceeds bounded record size");
                }
                bounded_write(&mut writer, &mut written, &bytes)?;
                bounded_write(&mut writer, &mut written, b"\n")?;
            }
            writer.flush()?;
            writer.get_ref().sync_all()?;
            Ok(())
        });
        Ok(Self {
            sender: Arc::new(JournalSender(sender)),
            worker,
        })
    }
    fn blocking_record(&self, event: Value) -> Result<()> {
        self.sender
            .0
            .send(JournalCommand::Record(event))
            .map_err(|_| eyre!("diagnostic writer failed"))
    }
    fn flush_before_collection(&self) -> Result<()> {
        let (acknowledgment, completed) = mpsc::sync_channel(1);
        self.sender
            .0
            .send(JournalCommand::Barrier(acknowledgment))
            .map_err(|_| eyre!("diagnostic writer failed before collection"))?;
        completed
            .recv()
            .map_err(|_| eyre!("scheduled request journal did not reach durable storage"))
    }
    fn finish(self) -> Result<()> {
        drop(self.sender);
        self.worker
            .join()
            .map_err(|_| eyre!("diagnostic writer panicked"))?
    }
}

fn classify_observation(
    expected: TransactionHash,
    response: &PipelineTransactionStatusResponse,
) -> Result<Option<u64>> {
    if response.hash != expected.to_string() || response.scope != "global" {
        bail!("status response does not bind exact requested hash/global scope");
    }
    match (
        response.status.kind.as_str(),
        response.resolved_from.as_str(),
    ) {
        ("Applied", "state") => response
            .status
            .block_height
            .filter(|height| *height > 0)
            .map(Some)
            .ok_or_else(|| eyre!("StateApplied has no positive authoritative height")),
        ("Rejected" | "Expired", "state") => {
            bail!("accepted transaction reached fixed state terminal failure")
        }
        ("Queued" | "Approved" | "Committed", "cache" | "queue" | "state")
        | ("Applied" | "Rejected" | "Expired", "cache" | "queue") => Ok(None),
        _ => bail!("unknown transaction status or provenance"),
    }
}

struct Submission {
    offer_ns: Option<i64>,
    response_ns: i64,
    result: Result<TransactionHash>,
}
#[derive(Clone, Copy)]
struct Offered {
    index: usize,
    offset: i64,
}

fn retain_failure(record: &mut Record, failure: &mut Option<String>, message: impl Into<String>) {
    let message = message.into();
    if record.failure.is_none() {
        record.failure = Some(message.clone());
    }
    if failure.is_none() {
        *failure = Some(message);
    }
}
fn accept_offer(
    record: &mut Record,
    offered: Offered,
    active: &mut BTreeSet<usize>,
    polls: &mut BinaryHeap<Reverse<(i64, usize)>>,
) -> Result<()> {
    if let Some(previous) = record.offer_ns {
        if previous != offered.offset {
            bail!("duplicate offer has conflicting observation time");
        }
        return Ok(());
    }
    record.offer_ns = Some(offered.offset);
    active.insert(offered.index);
    polls.push(Reverse((offered.offset, offered.index)));
    Ok(())
}

fn complete_preparation<P, R: Recorder>(
    index: usize,
    result: Result<Prepared<P>>,
    records: &mut [Record],
    prepared: &mut BTreeMap<usize, Prepared<P>>,
    seen_hashes: &mut BTreeSet<TransactionHash>,
    recorder: &R,
    now: i64,
    failure: &mut Option<String>,
) {
    match result {
        Ok(payload) => {
            records[index].hash = Some(payload.hash);
            if !seen_hashes.insert(payload.hash) {
                retain_failure(
                    &mut records[index],
                    failure,
                    "prepared transaction hash is duplicated within the run",
                );
            }
            if let Err(error) = recorder.record(norito::json!({"event": "prepared", "index": index,
                "hash": (payload.hash.to_string()), "offset_ns": now}))
            {
                retain_failure(&mut records[index], failure, error.to_string());
            }
            prepared.insert(index, payload);
        }
        Err(_) => retain_failure(
            &mut records[index],
            failure,
            "preparation failed; external error detail is not retained",
        ),
    }
}

fn complete_submission<R: Recorder>(
    index: usize,
    completed: Submission,
    records: &mut [Record],
    schedule: &Schedule,
    cohort: Cohort,
    recorder: &R,
    active: &mut BTreeSet<usize>,
    outstanding: &mut BTreeSet<usize>,
    polls: &mut BinaryHeap<Reverse<(i64, usize)>>,
    failure: &mut Option<String>,
) -> Result<()> {
    if let Some(offset) = completed.offer_ns {
        accept_offer(
            &mut records[index],
            Offered { index, offset },
            active,
            polls,
        )?;
    }
    records[index].submission_finished = true;
    if completed.offer_ns.is_none() {
        outstanding.remove(&index);
    }
    match completed.result {
        Ok(hash)
            if Some(hash) == records[index].hash
                && records[index]
                    .offer_ns
                    .is_some_and(|offer| offer <= completed.response_ns)
                && schedule.within_deadline(cohort, completed.response_ns) =>
        {
            records[index].acknowledgment_ns = Some(completed.response_ns);
            if let Err(error) = recorder.record(norito::json!({"event": "accepted", "index": index,
                            "hash": (hash.to_string()), "offset_ns": (completed.response_ns)}))
            {
                retain_failure(&mut records[index], failure, error.to_string());
            }
        }
        Ok(_) => retain_failure(
            &mut records[index],
            failure,
            "submission acknowledgment hash/time does not bind exact offered request",
        ),
        Err(_) => retain_failure(
            &mut records[index],
            failure,
            "submission failed or ambiguous; no replay; external error detail is not retained",
        ),
    }
    if records[index].settled() {
        active.remove(&index);
        outstanding.remove(&index);
    }
    Ok(())
}

fn status_diagnostic(
    index: usize,
    offset: i64,
    expected: TransactionHash,
    response: &PipelineTransactionStatusResponse,
) -> Value {
    // Even a failed decoder or hostile test transport must not put arbitrary
    // remote text into durable evidence. Keep exact identity-match observations
    // and bounded status vocabulary; qualification still uses the full response.
    let source = match response.resolved_from.as_str() {
        "state" => "state",
        "cache" => "cache",
        "queue" => "queue",
        _ => "unknown",
    };
    let status = match response.status.kind.as_str() {
        "Queued" => "Queued",
        "Approved" => "Approved",
        "Committed" => "Committed",
        "Applied" => "Applied",
        "Rejected" => "Rejected",
        "Expired" => "Expired",
        _ => "unknown",
    };
    norito::json!({"event": "status", "index": index, "offset_ns": offset,
        "expected_hash": (expected.to_string()), "hash_matches": (response.hash == expected.to_string()),
        "global_scope_matches": (response.scope == "global"), "resolved_from": source,
        "status": status, "block_height": (response.status.block_height)})
}

fn complete_observation<R: Recorder>(
    index: usize,
    offset: i64,
    result: Result<Option<PipelineTransactionStatusResponse>>,
    records: &mut [Record],
    schedule: &Schedule,
    cohort: Cohort,
    recorder: &R,
    active: &mut BTreeSet<usize>,
    outstanding: &mut BTreeSet<usize>,
    polls: &mut BinaryHeap<Reverse<(i64, usize)>>,
    poll_ns: i64,
    failure: &mut Option<String>,
) -> Result<()> {
    let hash = records[index]
        .hash
        .ok_or_else(|| eyre!("observation lost its exact hash"))?;
    let outcome = result.and_then(|response| {
        if let Some(response) = response {
            recorder.record(status_diagnostic(index, offset, hash, &response))?;
            classify_observation(hash, &response)
        } else {
            recorder.record(
                norito::json!({"event": "status_missing", "index": index, "offset_ns": offset,
                            "hash": (hash.to_string())}),
            )?;
            Ok(None)
        }
    });
    match outcome {
        Ok(Some(height))
            if records[index].offer_ns.is_some_and(|offer| offset > offer)
                && schedule.within_deadline(cohort, offset) =>
        {
            records[index].applied = Some((offset, height));
            if records[index].settled() {
                active.remove(&index);
                outstanding.remove(&index);
            }
        }
        Ok(Some(_)) => retain_failure(
            &mut records[index],
            failure,
            "StateApplied observation is outside its exact offer/deadline window",
        ),
        Ok(None) => polls.push(Reverse((offset.saturating_add(poll_ns), index))),
        Err(_) => retain_failure(
            &mut records[index],
            failure,
            "authoritative observation failed; external error detail is not retained",
        ),
    }
    Ok(())
}

async fn collect_phase<B: Backend, C: Clock, R: Recorder>(
    backend: Arc<B>,
    clock: Arc<C>,
    recorder: Arc<R>,
    schedule: &Schedule,
    bounds: &Bounds,
    records: &mut [Record],
    range: std::ops::Range<usize>,
    cohort: Cohort,
    seen_hashes: &mut BTreeSet<TransactionHash>,
) -> Result<()> {
    let deadline = schedule.deadline(cohort);
    let mut preparations: Work<Result<Prepared<B::Payload>>> = Work::new();
    let mut submissions: Work<Submission> = Work::new();
    let mut observations: Work<(i64, Result<Option<PipelineTransactionStatusResponse>>)> =
        Work::new();
    let mut prepared = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut outstanding = BTreeSet::new();
    let mut polls = BinaryHeap::new();
    let (offer_tx, mut offer_rx) = tokio::sync::mpsc::channel::<Offered>(bounds.submissions * 2);
    let mut next_prepare = range.start;
    let mut next_offer = range.start;
    let mut failure = None;

    loop {
        // Completion timestamps are captured inside the futures. Process already
        // ready work before evaluating a fixed-slot capacity or phase boundary.
        while let Ok(offered) = offer_rx.try_recv() {
            accept_offer(
                &mut records[offered.index],
                offered,
                &mut active,
                &mut polls,
            )?;
        }
        while let Some(Some((index, completed))) = submissions.next().now_or_never() {
            complete_submission(
                index,
                completed,
                records,
                schedule,
                cohort,
                recorder.as_ref(),
                &mut active,
                &mut outstanding,
                &mut polls,
                &mut failure,
            )?;
        }
        while let Some(Some((index, (offset, result)))) = observations.next().now_or_never() {
            complete_observation(
                index,
                offset,
                result,
                records,
                schedule,
                cohort,
                recorder.as_ref(),
                &mut active,
                &mut outstanding,
                &mut polls,
                bounds.poll_ns,
                &mut failure,
            )?;
        }
        let now = clock.now();
        let closing = now >= deadline;
        if failure.is_none() && !closing {
            while next_prepare < range.end
                && preparations.len() < bounds.preparations
                && next_prepare - next_offer < bounds.lookahead
                && records[next_prepare]
                    .plan
                    .scheduled_offset_ns
                    .saturating_sub(bounds.ahead_ns)
                    <= now
            {
                let index = next_prepare;
                let future = backend.clone().prepare(records[index].plan.clone());
                preparations.push(async move { (index, future.await) }.boxed());
                next_prepare += 1;
            }
            // A completed signer can become ready on the offer timer itself. Consume
            // ready results before deciding whether this original slot is usable.
            while let Some(Some((index, result))) = preparations.next().now_or_never() {
                complete_preparation(
                    index,
                    result,
                    records,
                    &mut prepared,
                    seen_hashes,
                    recorder.as_ref(),
                    clock.now(),
                    &mut failure,
                );
            }
            if failure.is_none()
                && next_offer < range.end
                && records[next_offer].plan.scheduled_offset_ns <= now
                && (prepared.contains_key(&next_offer)
                    || now
                        >= records[next_offer]
                            .plan
                            .scheduled_offset_ns
                            .saturating_add(schedule.lag_ns))
            {
                let index = next_offer;
                let can_offer = schedule.offer(&records[index].plan, now).and_then(|()| {
                    if submissions.len() >= bounds.submissions || outstanding.len() >= bounds.in_flight {
                        bail!("bounded submission or outstanding observation capacity exhausted at fixed offer");
                    }
                    if !prepared.contains_key(&index) { bail!("signed transaction was not ready at its fixed offer"); }
                    Ok(())
                });
                if let Err(error) = can_offer {
                    retain_failure(&mut records[index], &mut failure, error.to_string());
                } else {
                    let payload = prepared
                        .remove(&index)
                        .ok_or_else(|| eyre!("prepared request disappeared"))?;
                    let plan = records[index].plan.clone();
                    let backend = backend.clone();
                    let clock = clock.clone();
                    let recorder = recorder.clone();
                    let offer_tx = offer_tx.clone();
                    let schedule = schedule.clone();
                    submissions.push(
                        async move {
                            let start = (|| -> Result<i64> {
                                let permit = offer_tx
                                    .try_reserve()
                                    .map_err(|_| eyre!("bounded offer event queue unavailable"))?;
                                let offset = clock.now();
                                schedule.offer(&plan, offset)?;
                                recorder
                                    .record(norito::json!({"event": "offer", "index": index,
                                "hash": (payload.hash.to_string()), "offset_ns": offset}))?;
                                permit.send(Offered { index, offset });
                                Ok(offset)
                            })();
                            let (offer_ns, result) = match start {
                                Ok(offset) => (Some(offset), backend.submit(payload).await),
                                Err(error) => (None, Err(error)),
                            };
                            (
                                index,
                                Submission {
                                    offer_ns,
                                    response_ns: clock.now(),
                                    result,
                                },
                            )
                        }
                        .boxed(),
                    );
                    outstanding.insert(index);
                    next_offer += 1;
                }
            }
        }

        while !closing && observations.len() < bounds.observations {
            let Some(Reverse((due, index))) = polls.peek().copied() else {
                break;
            };
            if due > now {
                break;
            }
            polls.pop();
            if !active.contains(&index) || records[index].applied.is_some() {
                continue;
            }
            let hash = records[index]
                .hash
                .ok_or_else(|| eyre!("offered request lost its exact hash"))?;
            let account = records[index].plan.account_index;
            records[index].attempts += 1;
            let future = backend.clone().observe(account, hash);
            let clock = clock.clone();
            observations.push(
                async move {
                    let result = future.await;
                    (index, (clock.now(), result))
                }
                .boxed(),
            );
        }

        if failure.is_none()
            && next_offer == range.end
            && active.is_empty()
            && submissions.is_empty()
            && observations.is_empty()
            && preparations.is_empty()
            && cohort == Cohort::Warmup
        {
            break;
        }
        if failure.is_some()
            && active.is_empty()
            && submissions.is_empty()
            && observations.is_empty()
            && preparations.is_empty()
        {
            break;
        }
        let mut wake = deadline;
        if failure.is_none() && next_offer < range.end {
            let scheduled = records[next_offer].plan.scheduled_offset_ns;
            let offer_wake = if scheduled <= now && !prepared.contains_key(&next_offer) {
                scheduled.saturating_add(schedule.lag_ns)
            } else {
                scheduled
            };
            wake = wake.min(offer_wake);
        }
        if failure.is_none()
            && next_prepare < range.end
            && preparations.len() < bounds.preparations
            && next_prepare - next_offer < bounds.lookahead
        {
            wake = wake.min(
                records[next_prepare]
                    .plan
                    .scheduled_offset_ns
                    .saturating_sub(bounds.ahead_ns),
            );
        }
        if observations.len() < bounds.observations
            && let Some(Reverse((due, _))) = polls.peek()
        {
            wake = wake.min(*due);
        }
        let timer = clock.clone().sleep(wake.max(now));
        tokio::select! {
            biased;
            Some(offered) = offer_rx.recv() => {
                accept_offer(&mut records[offered.index], offered, &mut active, &mut polls)?;
            }
            Some((index, completed)) = submissions.next(), if !submissions.is_empty() => {
                complete_submission(index, completed, records, schedule, cohort, recorder.as_ref(),
                    &mut active, &mut outstanding, &mut polls, &mut failure)?;
            }
            Some((index, result)) = preparations.next(), if !preparations.is_empty() => {
                complete_preparation(index, result, records, &mut prepared, seen_hashes,
                    recorder.as_ref(), clock.now(), &mut failure);
            }

            Some((index, (offset, result))) = observations.next(), if !observations.is_empty() => {
                complete_observation(index, offset, result, records, schedule, cohort, recorder.as_ref(),
                    &mut active, &mut outstanding, &mut polls, bounds.poll_ns, &mut failure)?;
            }
            reached = timer => {
                if closing {
                    // The biased arms above consume all already-ready completions,
                    // retaining their captured timestamps, before final accounting.
                    // Measurement accepts exactly T+D; warmup still requires <0.
                    for index in range.clone() {
                        if !records[index].settled() {
                            retain_failure(&mut records[index], &mut failure,
                                "scheduled request unsettled at fixed phase deadline");
                        }
                    }
                    break;
                }
                clock.timer_reached(reached);
            }
        }
    }
    if let Some(failure) = failure {
        bail!(failure);
    }
    for index in range {
        if !records[index].settled() || records[index].failure.is_some() {
            bail!("phase has an incomplete scheduled request");
        }
    }
    Ok(())
}

fn publish_trace(path: &Path, args: &Args, records: &[Record]) -> Result<()> {
    // The no-clobber hard-link publication happens only after complete serialization
    // and fsync. On failure the owned partial file remains diagnostic evidence.
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| eyre!("trace output name must be UTF-8"))?;
    let stage = path.with_file_name(format!("{name}.collecting"));
    let file = new_owned_file(&stage)?;
    let mut writer = BufWriter::new(file);
    let mut written = 0;
    let header = norito::json!({"schema": TRACE_SCHEMA, "pair_index": (args.pair_index),
        "variant": (args.variant.text()), "seed": (args.seed),
        "clock": "monotonic_nanoseconds_relative_to_measurement_start",
        "logical_id_derivation": LOGICAL_DERIVATION,
        "transaction_hash_source": "iroha_data_model::transaction::SignedTransaction::hash"});
    let mut bytes = json::to_vec(&header)?;
    if bytes.pop() != Some(b'}') {
        bail!("trace header did not encode as a JSON object");
    }
    bounded_write(&mut writer, &mut written, &bytes)?;
    bounded_write(&mut writer, &mut written, b",\"transactions\":[")?;
    for (index, record) in records.iter().enumerate() {
        if index != 0 {
            bounded_write(&mut writer, &mut written, b",")?;
        }
        bounded_write(
            &mut writer,
            &mut written,
            &json::to_vec(&record.trace_value()?)?,
        )?;
    }
    bounded_write(&mut writer, &mut written, b"]}\n")?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    std::fs::hard_link(&stage, path)
        .wrap_err("trace publication requires an absent destination")?;
    std::fs::remove_file(&stage)?;
    File::open(path.parent().ok_or_else(|| eyre!("trace parent missing"))?)?.sync_all()?;
    Ok(())
}

impl Run for Args {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if context.input_instructions() || context.output_instructions() {
            bail!("transaction load does not accept instruction stdin/stdout modes");
        }
        if !(1..=5).contains(&self.pair_index)
            || self.seed.len() != 64
            || !self
                .seed
                .bytes()
                .all(|ch| ch.is_ascii_digit() || (b'a'..=b'f').contains(&ch))
        {
            bail!("load requires a five-pair index and canonical lowercase SHA-256 seed");
        }
        if self.trace_out == self.diagnostic_out || self.trace_out.try_exists()? {
            bail!("trace and diagnostic outputs must be distinct, absent files");
        }
        let schedule = Schedule::from_args(&self)?;
        let bounds = Bounds::from_args(&self)?;
        let resource_plan = resource::Plan::new(&self.resource, &schedule, &bounds)?;
        let configs = if self.account_configs.is_empty() {
            vec![context.config().clone()]
        } else {
            if self.account_configs.len() > MAX_ACCOUNTS {
                bail!("account pool exceeds {MAX_ACCOUNTS}");
            }
            self.account_configs
                .iter()
                .map(|path| {
                    Config::load_file(path)
                        .map_err(|_| eyre!("account configuration cannot be loaded"))
                })
                .collect::<Result<Vec<_>>>()?
        };
        let mut clients = Vec::new();
        let mut accounts = Vec::new();
        let mut authorities = BTreeSet::new();
        for config in configs {
            if config.network_id != context.config().network_id {
                bail!("account pool changes the selected network");
            }
            let client = Client::builder(config)
                .build()
                .map_err(|_| eyre!("account client configuration cannot be validated"))?;
            let account = client
                .account_client()
                .map_err(|_| eyre!("account context cannot be bound"))?;
            if !authorities.insert(account.authority().clone()) {
                bail!("account pool duplicates a signing authority");
            }
            clients.push(client);
            accounts.push(account);
        }
        workload::validate_schedule(&schedule, accounts.len())?;
        let mut records = schedule.plan(&self.seed, accounts.len())?;
        // Deployment evidence owns peer identity. Endpoint paths and transport errors
        // can carry secrets, so the journal binds only the public account pool.
        let public_accounts: Vec<Value> = accounts
            .iter()
            .map(|account| norito::json!({"authority": (account.authority().to_string())}))
            .collect();
        let backend = Arc::new(SdkBackend {
            clients,
            accounts,
            metadata: context.transaction_metadata().cloned().unwrap_or_default(),
            fee: context.transaction_fee_payment()?,
        });
        let journal = Journal::start(&self.diagnostic_out, self.journal_capacity)?;
        journal.blocking_record(norito::json!({"event": "plan", "schema": "iroha.sumeragi_v2.multilane_scaling.collector_journal.v1",
            "pair_index": (self.pair_index), "variant": (self.variant.text()), "seed": (self.seed),
            "accounts": public_accounts, "account_selection": (workload::ACCOUNT_SELECTION),
            "workload": (workload::WORKLOAD_ID), "max_effects_per_account": (workload::MAX_EFFECTS_PER_ACCOUNT),
            "scheduled_requests": (records.len()), "warmup_ns": (schedule.warmup_ns), "measurement_ns": (schedule.measurement_ns),
            "drain_ns": (schedule.drain_ns), "submission_lag_bound_ns": (schedule.lag_ns),
            "preparation_lookahead": (bounds.lookahead), "preparation_concurrency": (bounds.preparations),
            "preparation_ahead_ns": (bounds.ahead_ns), "max_submissions": (bounds.submissions),
            "max_in_flight": (bounds.in_flight), "max_status_requests": (bounds.observations), "poll_interval_ns": (bounds.poll_ns)}))?;
        for (index, record) in records.iter().enumerate() {
            journal.blocking_record(
                norito::json!({"event": "scheduled", "index": index, "plan": (record.plan.value())}),
            )?;
        }
        journal.flush_before_collection()?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(bounds.preparations)
            .enable_all()
            .build()?;
        let outcome = runtime.block_on(async {
            for client in &backend.clients {
                client.refresh_capabilities().await.map_err(|_| {
                    eyre!("capability preflight failed; external error detail is not retained")
                })?;
            }
            let baselines = tokio::task::block_in_place(|| {
                workload::preflight(backend.as_ref(), &records, journal.sender.as_ref())
            })?;
            let resource_session = resource::Session::preflight(
                &self.resource,
                resource_plan,
                journal.sender.as_ref(),
            )
            .await?;
            let initial_offset = schedule
                .start(Cohort::Warmup)
                .checked_sub(bounds.ahead_ns)
                .ok_or_else(|| eyre!("collector clock origin overflows"))?;
            let clock = Arc::new(LiveClock {
                started: Instant::now(),
                initial_offset,
            });
            let recorder = journal.sender.clone();
            recorder.record(
                norito::json!({"event": "clock_started", "initial_offset_ns": initial_offset}),
            )?;
            let warmup_end = schedule.count(Cohort::Warmup)?;
            let total = records.len();
            let mut hashes = BTreeSet::new();
            // Both futures run to completion: a failed probe must not cancel an
            // in-flight submission or erase its bounded transaction drain.
            let transactions = async {
                collect_phase(
                    backend.clone(),
                    clock.clone(),
                    recorder.clone(),
                    &schedule,
                    &bounds,
                    &mut records,
                    0..warmup_end,
                    Cohort::Warmup,
                    &mut hashes,
                )
                .await?;
                collect_phase(
                    backend.clone(),
                    clock.clone(),
                    recorder.clone(),
                    &schedule,
                    &bounds,
                    &mut records,
                    warmup_end..total,
                    Cohort::Measurement,
                    &mut hashes,
                )
                .await?;
                Ok::<(), eyre::Report>(())
            };
            let sampling = resource_session.collect(clock.clone(), recorder.clone(), resource_plan);
            let (transaction_outcome, resource_outcome) = futures::join!(transactions, sampling);
            transaction_outcome?;
            recorder.record(norito::json!({"event": "workload_postconditions_started"}))?;
            tokio::task::block_in_place(|| {
                workload::verify(backend.as_ref(), &records, &baselines, recorder.as_ref())
            })?;
            resource_outcome
        });
        for record in &mut records {
            if !record.settled() && record.failure.is_none() {
                record.failure = Some(
                    if record.offer_ns.is_some() {
                        "offered request remains unsettled after collector failure"
                    } else {
                        "scheduled request was not offered after collector failure"
                    }
                    .to_owned(),
                );
            }
            journal.blocking_record(record.diagnostic_value())?;
        }
        journal.blocking_record(
            norito::json!({"event": "collection_finished", "passed": (outcome.is_ok()),
            "failure": (outcome.as_ref().err().map(ToString::to_string))}),
        )?;
        journal.finish()?;
        outcome?;
        publish_trace(&self.trace_out, &self, &records)?;
        context.println(format!(
            "Recorded {} exact scheduled transaction outcomes to {}",
            records.len(),
            self.trace_out.display()
        ))
    }
}

#[cfg(test)]
mod tests;

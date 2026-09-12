//! Mandatory fixed-cadence resource observations on the collector's own clock.
//!
//! A separate bounded pipe worker owns exactly one child. The transaction engine
//! and sampler share a clock but neither advances the other's fixed schedule.
//! Raw peer bodies remain in immutable captures; journal events contain only
//! authenticated small manifest references and collector-owned clock brackets.

use super::*;

#[cfg(unix)]
mod ipc;
#[cfg(test)]
mod tests;

const REQUEST_SCHEMA: &str = "iroha.sumeragi_v2.resource_probe.request.v1";
pub(super) const RESPONSE_SCHEMA: &str = "iroha.sumeragi_v2.resource_probe.response.v1";
const CAPTURE_SCHEMA: &str = "iroha.sumeragi_v2.resource_probe.capture.v1";
pub(super) const MAX_IPC_BYTES: usize = 16 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_SAMPLES: u64 = 100_000;

/// Required private probe inputs and exact resource sampling geometry.
#[derive(clap::Args, Debug)]
#[group(id = "TransactionLoadResourceArgs")]
pub(super) struct Args {
    /// Absolute interpreter executable; invoked directly, without a shell.
    #[arg(long, value_name = "PATH")]
    resource_program: PathBuf,
    /// Absolute fixed resource_probe_worker.py implementation.
    #[arg(long, value_name = "PATH")]
    resource_worker: PathBuf,
    /// Existing owner-only runtime probe configuration; never copied to evidence.
    #[arg(long, value_name = "PATH")]
    resource_config: PathBuf,
    /// Independently supplied SHA-256 of the canonical complete public run budget.
    #[arg(long)]
    pub(super) resource_budget_sha256: String,
    /// Absent directory for new owner-only, immutable probe captures.
    #[arg(long, value_name = "PATH")]
    resource_capture_dir: PathBuf,
    /// Exact sampling period, dividing both measurement and drain durations.
    #[arg(long)]
    resource_interval_ms: u64,
    /// Whole request/response deadline, at most half one sampling period.
    #[arg(long)]
    resource_timeout_ms: u64,
    /// Maximum start lag, at most one quarter of one sampling period.
    #[arg(long)]
    resource_max_start_lag_ms: u64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Plan {
    pub(super) interval_ns: i64,
    timeout_ns: i64,
    start_lag_ns: i64,
    final_offset_ns: i64,
    samples: u64,
    lifetime: Duration,
}
impl Plan {
    pub(super) fn new(args: &Args, schedule: &Schedule, bounds: &Bounds) -> Result<Self> {
        fn milliseconds(value: u64) -> Result<i64> {
            i64::try_from(value)
                .ok()
                .and_then(|value| value.checked_mul(1_000_000))
                .ok_or_else(|| eyre!("resource timing arithmetic overflow"))
        }
        let interval_ns = milliseconds(args.resource_interval_ms)?;
        let timeout_ns = milliseconds(args.resource_timeout_ms)?;
        let start_lag_ns = milliseconds(args.resource_max_start_lag_ms)?;
        if !(2_000_000..=60 * NS).contains(&interval_ns)
            || !(1_000_000..=30 * NS).contains(&timeout_ns)
            || timeout_ns > interval_ns / 2
            || start_lag_ns > interval_ns / 4
            || schedule.measurement_ns / interval_ns < 20
            || schedule.measurement_ns % interval_ns != 0
            || schedule.drain_ns % interval_ns != 0
        {
            bail!("invalid exact resource sampling geometry");
        }
        let final_offset_ns = schedule.deadline(Cohort::Measurement);
        let samples = u64::try_from(final_offset_ns / interval_ns + 1)
            .map_err(|_| eyre!("resource sample count overflow"))?;
        if !(2..=MAX_SAMPLES).contains(&samples) {
            bail!("resource sample count exceeds bounded protocol");
        }
        // Includes preflight, the full warmup, both drains, preparation lead,
        // the endpoint sample, and finish. This is only a child lifetime bound;
        // sample start/end offsets always come from Clock.
        let lifetime_ns = final_offset_ns
            .checked_sub(schedule.start(Cohort::Warmup))
            .and_then(|v| v.checked_add(bounds.ahead_ns))
            .and_then(|v| v.checked_add(3 * timeout_ns))
            .and_then(|v| v.checked_add(start_lag_ns))
            .ok_or_else(|| eyre!("resource child lifetime overflow"))?;
        Ok(Self {
            interval_ns,
            timeout_ns,
            start_lag_ns,
            final_offset_ns,
            samples,
            lifetime: Duration::from_nanos(lifetime_ns as u64),
        })
    }
    fn request(self, kind: Kind, sequence: u64) -> Request {
        Request {
            kind,
            sequence,
            timeout_ms: (self.timeout_ns / 1_000_000) as u64,
        }
    }
    fn value(self) -> Value {
        norito::json!({"interval_ns": (self.interval_ns), "response_deadline_ns": (self.timeout_ns),
            "max_start_lag_ns": (self.start_lag_ns), "first_offset_ns": 0,
            "final_offset_ns": (self.final_offset_ns), "sample_count": (self.samples)})
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Admit,
    Preflight,
    Sample,
    Finish,
}
impl Kind {
    fn text(self) -> &'static str {
        match self {
            Self::Admit => "admit",
            Self::Preflight => "preflight",
            Self::Sample => "sample",
            Self::Finish => "finish",
        }
    }
    fn manifest_name(self, sequence: u64) -> String {
        format!("{}-{sequence:010}.json", self.text())
    }
}
#[derive(Clone, Copy, Debug)]
struct Request {
    kind: Kind,
    sequence: u64,
    timeout_ms: u64,
}
impl Request {
    fn line(self) -> Result<Vec<u8>> {
        let mut bytes = json::to_vec(&norito::json!({"schema": REQUEST_SCHEMA,
            "kind": (self.kind.text()), "sequence": (self.sequence), "timeout_ms": (self.timeout_ms)}))
            .map_err(|_| eyre!("resource request encoding failed"))?;
        bytes.push(b'\n');
        if bytes.len() > MAX_IPC_BYTES {
            bail!("resource request exceeds frame bound");
        }
        Ok(bytes)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Manifest {
    name: String,
    sha256: String,
    bytes: u64,
}
impl Manifest {
    fn value(&self) -> Value {
        norito::json!({"name": (self.name), "sha256": (self.sha256), "bytes": (self.bytes)})
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Complete,
    Unavailable,
    Failed,
}
impl Outcome {
    fn text(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Unavailable => "unavailable",
            Self::Failed => "failed",
        }
    }
}
#[derive(Clone, Debug)]
struct Response {
    outcome: Outcome,
    manifest: Option<Manifest>,
}
fn parse_response(request: Request, line: &[u8]) -> Result<Response> {
    if request.kind == Kind::Admit {
        bail!("admission requires its typed receipt decoder");
    }
    if line.is_empty()
        || line.len() > MAX_IPC_BYTES
        || line.last() != Some(&b'\n')
        || line[..line.len() - 1].contains(&b'\n')
    {
        bail!("resource response is not one bounded line");
    }
    let value: Value =
        json::from_slice(line).map_err(|_| eyre!("resource response is not valid JSON"))?;
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("resource response is not an object"))?;
    if object.len() != 5
        || object.get("schema").and_then(Value::as_str) != Some(RESPONSE_SCHEMA)
        || object.get("kind").and_then(Value::as_str) != Some(request.kind.text())
        || object.get("sequence").and_then(Value::as_u64) != Some(request.sequence)
    {
        bail!("resource response identity mismatch");
    }
    let outcome = match object.get("outcome").and_then(Value::as_str) {
        Some("complete") => Outcome::Complete,
        Some("unavailable") => Outcome::Unavailable,
        Some("failed") => Outcome::Failed,
        _ => bail!("resource response outcome is invalid"),
    };
    let manifest_value = object
        .get("manifest")
        .ok_or_else(|| eyre!("resource manifest field missing"))?;
    let manifest = if manifest_value == &Value::Null {
        None
    } else {
        let reference = manifest_value
            .as_object()
            .ok_or_else(|| eyre!("resource manifest reference is invalid"))?;
        let name = reference
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre!("resource manifest name missing"))?;
        let sha256 = reference
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre!("resource manifest digest missing"))?;
        let bytes = reference
            .get("bytes")
            .and_then(Value::as_u64)
            .ok_or_else(|| eyre!("resource manifest size missing"))?;
        if reference.len() != 3
            || name != request.kind.manifest_name(request.sequence)
            || sha256.len() != 64
            || !sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !(1..=MAX_MANIFEST_BYTES).contains(&bytes)
            || request.kind == Kind::Finish
        {
            bail!("resource manifest reference exceeds exact identity or bounds");
        }
        Some(Manifest {
            name: name.to_owned(),
            sha256: sha256.to_owned(),
            bytes,
        })
    };
    if (request.kind != Kind::Finish && outcome != Outcome::Failed && manifest.is_none())
        || (request.kind == Kind::Finish && manifest.is_some())
    {
        bail!("resource response omitted required capture");
    }
    Ok(Response { outcome, manifest })
}

trait Probe: Send + Sync {
    fn exchange(&self, request: Request) -> BoxFuture<'_, Result<Response>>;
    fn abort(&self);
}

pub(super) struct Session {
    #[cfg(unix)]
    probe: ipc::ChildProbe,
}
impl Session {
    pub(super) fn check_admission_deadline(&self) -> Result<()> {
        #[cfg(unix)]
        {
            self.probe.check_admission_deadline()
        }
        #[cfg(not(unix))]
        {
            bail!("bounded resource probe requires supported descriptor ownership");
        }
    }
    pub(super) async fn admit(
        args: &Args,
        plan: Plan,
        expected: allocation::Expected,
    ) -> Result<(Self, allocation::Writers)> {
        #[cfg(unix)]
        {
            let (probe, writers) = ipc::ChildProbe::admit(args, plan, expected).await?;
            Ok((Self { probe }, writers))
        }
        #[cfg(not(unix))]
        {
            let _ = (args, plan, expected);
            bail!("bounded resource probe requires supported descriptor ownership");
        }
    }
    pub(super) async fn preflight(&self, plan: Plan, recorder: &impl Recorder) -> Result<()> {
        #[cfg(unix)]
        {
            let response = self
                .probe
                .exchange(plan.request(Kind::Preflight, 0))
                .await?;
            recorder.record(norito::json!({"event": "resource_preflight", "sequence": 0,
                "outcome": (response.outcome.text()), "manifest": (response.manifest.as_ref().map(Manifest::value)),
                "sampling": (plan.value())}))?;
            if response.outcome != Outcome::Complete {
                bail!("resource preflight did not establish complete observations");
            }
            Ok(())
        }
        #[cfg(not(unix))]
        {
            let _ = (plan, recorder);
            bail!("bounded resource probe requires supported descriptor ownership");
        }
    }
    pub(super) async fn collect<C: Clock, R: Recorder>(
        &self,
        clock: Arc<C>,
        recorder: Arc<R>,
        plan: Plan,
    ) -> Result<()> {
        #[cfg(unix)]
        {
            collect(&self.probe, clock, recorder, plan).await
        }
        #[cfg(not(unix))]
        {
            let _ = (clock, recorder, plan);
            bail!("bounded resource probe requires supported descriptor ownership");
        }
    }
}

async fn wait_until<C: Clock>(clock: Arc<C>, target: i64) {
    while clock.now() < target {
        let reached = clock.clone().sleep(target).await;
        clock.timer_reached(reached);
    }
}

async fn timed_exchange<P: Probe, C: Clock>(
    probe: &P,
    clock: Arc<C>,
    request: Request,
    deadline: i64,
) -> Result<(Response, i64)> {
    let reply = probe.exchange(request).fuse();
    let timeout = wait_until(clock.clone(), deadline).fuse();
    futures::pin_mut!(reply, timeout);
    futures::select_biased! {
        result = reply => {
            let end = clock.now();
            if end >= deadline { bail!("resource response exceeded collector clock deadline"); }
            Ok((result?, end))
        },
        () = timeout => bail!("resource response exceeded collector clock deadline"),
    }
}

async fn collect<P: Probe, C: Clock, R: Recorder>(
    probe: &P,
    clock: Arc<C>,
    recorder: Arc<R>,
    plan: Plan,
) -> Result<()> {
    let result = async {
        for index in 0..plan.samples {
            let scheduled = i64::try_from(index).ok().and_then(|v| v.checked_mul(plan.interval_ns))
                .ok_or_else(|| eyre!("resource scheduled offset overflow"))?;
            wait_until(clock.clone(), scheduled).await;
            let start = clock.now();
            if start < scheduled || start - scheduled > plan.start_lag_ns {
                bail!("resource sample missed its fixed start bound");
            }
            let sequence = index + 1;
            recorder.record(norito::json!({"event": "resource_request", "kind": "sample", "sequence": sequence,
                "scheduled_offset_ns": scheduled, "start_offset_ns": start}))?;
            let deadline_origin = if sequence == plan.samples { scheduled.min(start) } else { start };
            // The endpoint observation may finish less than one timeout after
            // T+D. Start lag never extends the transaction drain or this cap.
            let deadline = deadline_origin.checked_add(plan.timeout_ns)
                .ok_or_else(|| eyre!("resource deadline overflow"))?;
            let (response, end) = timed_exchange(probe, clock.clone(), plan.request(Kind::Sample, sequence), deadline).await?;
            if end < start { bail!("resource clock moved backwards"); }
            recorder.record(norito::json!({"event": "resource_observation", "sequence": sequence,
                "scheduled_offset_ns": scheduled, "start_offset_ns": start, "end_offset_ns": end,
                "outcome": (response.outcome.text()), "manifest": (response.manifest.as_ref().map(Manifest::value))}))?;
            if response.outcome != Outcome::Complete { bail!("resource sample is incomplete"); }
        }
        let sequence = plan.samples + 1;
        let start = clock.now();
        recorder.record(norito::json!({"event": "resource_request", "kind": "finish", "sequence": sequence, "start_offset_ns": start}))?;
        let (response, end) = timed_exchange(probe, clock.clone(), plan.request(Kind::Finish, sequence),
            start.checked_add(plan.timeout_ns).ok_or_else(|| eyre!("resource finish deadline overflow"))?).await?;
        if response.outcome != Outcome::Complete || response.manifest.is_some() || end < start {
            bail!("resource probe did not close successfully");
        }
        recorder.record(norito::json!({"event": "resource_collection_finished", "sequence": sequence,
            "start_offset_ns": start, "end_offset_ns": end, "sampling": (plan.value())}))?;
        Ok(())
    }.await;
    if result.is_err() {
        probe.abort();
        recorder.record(norito::json!({"event": "resource_collection_failed", "failure": "bounded_resource_collection_failed"}))?;
    }
    result
}

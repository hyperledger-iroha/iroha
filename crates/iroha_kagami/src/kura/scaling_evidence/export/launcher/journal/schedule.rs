//! Derive the collector schedule solely from independently supplied launch inputs.

use super::*;

const NS: i64 = 1_000_000_000;

pub(super) struct Planned {
    pub(super) phase: WorkloadPhase,
    pub(super) sequence: usize,
    pub(super) logical_id: String,
    pub(super) scheduled_offset_ns: i64,
    pub(super) account_index: usize,
}
pub(super) fn phase_text(phase: WorkloadPhase) -> &'static str {
    match phase {
        WorkloadPhase::Warmup => "warmup",
        WorkloadPhase::Measurement => "measurement",
    }
}
impl Planned {
    pub(super) fn matches(&self, value: &Value) -> Result<()> {
        fields(
            value,
            &[
                "cohort",
                "sequence",
                "logical_id",
                "scheduled_offset_ns",
                "account_index",
            ],
        )?;
        equal_text(value, "cohort", phase_text(self.phase))?;
        equal_uint(value, "sequence", self.sequence)?;
        equal_text(value, "logical_id", &self.logical_id)?;
        equal_int(value, "scheduled_offset_ns", self.scheduled_offset_ns)?;
        equal_uint(value, "account_index", self.account_index)
    }
}
pub(super) struct Derived {
    pub(super) warmup: usize,
    pub(super) total: usize,
    pub(super) effects: usize,
    pub(super) origin: i64,
    pub(super) final_ns: i64,
    pub(super) samples: usize,
    account_offset: usize,
    period_numerator: u128,
}
pub(super) fn derive(expected: &JournalExpectations) -> Result<Derived> {
    digest(&expected.seed)?;
    ensure!(
        (1..=5).contains(&expected.pair_index),
        "independent pair must be one through five"
    );
    let accounts = &expected.accounts;
    ensure!(
        (4..=MAX_ACCOUNTS).contains(&accounts.len()) && accounts.len() % 4 == 0,
        "independent account pool must contain complete groups of four"
    );
    let mut identities = BTreeSet::new();
    let mut routes = BTreeSet::new();
    for account in accounts {
        ensure!(
            identities.insert(&account.authority) && account.authority.to_string().len() <= 2048,
            "duplicate or unbounded independent authority"
        );
        routes.insert((account.route.lane_id, account.route.dataspace_id));
    }
    let lanes = match expected.variant {
        JournalVariant::OneLane => 1,
        JournalVariant::FourLane => 4,
    };
    ensure!(
        routes.len() == lanes
            && routes
                .iter()
                .map(|(lane, _)| *lane)
                .collect::<BTreeSet<_>>()
                .len()
                == lanes,
        "independent variant route geometry mismatch"
    );
    for group in accounts.chunks_exact(4) {
        ensure!(
            group
                .iter()
                .map(|a| (a.route.lane_id, a.route.dataspace_id))
                .collect::<BTreeSet<_>>()
                == routes,
            "independent account group omits a selected route"
        );
    }
    let time = &expected.timing;
    let bounds = &expected.bounds;
    let sampling = &expected.sampling;
    ensure!(
        time.rate_numerator > 0
            && time.rate_denominator > 0
            && time.warmup_ns >= 0
            && time.measurement_ns > 0
            && (1..=300 * NS).contains(&time.drain_ns)
            && time.submission_lag_bound_ns >= 0,
        "invalid independent offer geometry"
    );
    let period_numerator = (NS as u128)
        .checked_mul(time.rate_denominator)
        .ok_or_else(|| eyre!("rate overflow"))?;
    ensure!(
        time.rate_numerator <= period_numerator
            && (time.submission_lag_bound_ns as u128)
                .checked_mul(4)
                .and_then(|n| n.checked_mul(time.rate_numerator))
                .is_some_and(|n| n <= period_numerator),
        "independent lag exceeds quarter arrival period"
    );
    for (value, maximum) in [
        (bounds.preparation_lookahead, 4096),
        (bounds.preparation_concurrency, 32),
        (bounds.max_submissions, 4096),
        (bounds.max_in_flight, 16384),
        (bounds.max_status_requests, 256),
        (bounds.max_requests, MAX_REQUESTS),
    ] {
        ensure!(
            (1..=maximum).contains(&value),
            "independent collector work bound invalid"
        );
    }
    ensure!(
        (1_000_000..=30 * NS).contains(&bounds.preparation_ahead_ns)
            && bounds.preparation_ahead_ns % 1_000_000 == 0
            && (1_000_000..=10 * NS).contains(&bounds.poll_interval_ns)
            && bounds.poll_interval_ns % 1_000_000 == 0,
        "independent preparation/poll duration invalid"
    );
    let count = |duration: i64| -> Result<usize> {
        let n = (duration as u128)
            .checked_mul(time.rate_numerator)
            .ok_or_else(|| eyre!("schedule count overflow"))?;
        Ok(usize::try_from(n.div_ceil(period_numerator))?)
    };
    let warmup = count(time.warmup_ns)?;
    let measurement = count(time.measurement_ns)?;
    let total = warmup
        .checked_add(measurement)
        .ok_or_else(|| eyre!("schedule count overflow"))?;
    ensure!(
        measurement > 0
            && warmup % accounts.len() == 0
            && measurement % accounts.len() == 0
            && total <= bounds.max_requests
            && total / accounts.len() <= 1024,
        "independent schedule exceeds complete account-pool rounds or effect bound"
    );
    let final_ns = time
        .measurement_ns
        .checked_add(time.drain_ns)
        .ok_or_else(|| eyre!("phase overflow"))?;
    let lead = time
        .warmup_ns
        .checked_add(time.drain_ns)
        .and_then(|n| n.checked_add(bounds.preparation_ahead_ns))
        .ok_or_else(|| eyre!("clock origin overflow"))?;
    ensure!(
        (2_000_000..=60 * NS).contains(&sampling.interval_ns)
            && (1_000_000..=30 * NS).contains(&sampling.response_deadline_ns)
            && sampling.response_deadline_ns <= sampling.interval_ns / 2
            && (0..=sampling.interval_ns / 4).contains(&sampling.max_start_lag_ns)
            && [
                sampling.interval_ns,
                sampling.response_deadline_ns,
                sampling.max_start_lag_ns
            ]
            .iter()
            .all(|v| v % 1_000_000 == 0)
            && time.measurement_ns / sampling.interval_ns >= 20
            && time.measurement_ns % sampling.interval_ns == 0
            && time.drain_ns % sampling.interval_ns == 0,
        "independent resource clock geometry invalid"
    );
    let samples = usize::try_from(final_ns / sampling.interval_ns + 1)?;
    ensure!(
        (2..=100_000).contains(&samples),
        "independent resource sample count invalid"
    );
    final_ns
        .checked_add(lead)
        .and_then(|n| n.checked_add(3 * sampling.response_deadline_ns))
        .and_then(|n| n.checked_add(sampling.max_start_lag_ns))
        .ok_or_else(|| eyre!("collector lifetime overflow"))?;
    let rotation =
        iroha_crypto::sha256(format!("gscale-account-offset-v1:{}", expected.seed).as_bytes());
    let account_offset = usize::try_from(
        u64::from_le_bytes(rotation[..8].try_into()?) % u64::try_from(accounts.len())?,
    )?;
    Ok(Derived {
        warmup,
        total,
        effects: total / accounts.len(),
        origin: -lead,
        final_ns,
        samples,
        account_offset,
        period_numerator,
    })
}
impl Derived {
    pub(super) fn plan(&self, expected: &JournalExpectations, index: usize) -> Result<Planned> {
        ensure!(
            index < self.total,
            "schedule index outside independent count"
        );
        let (phase, ordinal, start) = if index < self.warmup {
            (
                WorkloadPhase::Warmup,
                index,
                -expected.timing.warmup_ns - expected.timing.drain_ns,
            )
        } else {
            (WorkloadPhase::Measurement, index - self.warmup, 0)
        };
        let sequence = ordinal + 1;
        let offset = (ordinal as u128)
            .checked_mul(self.period_numerator)
            .ok_or_else(|| eyre!("schedule offset overflow"))?
            / expected.timing.rate_numerator;
        let scheduled_offset_ns = start
            .checked_add(i64::try_from(offset)?)
            .ok_or_else(|| eyre!("schedule offset overflow"))?;
        Ok(Planned {
            phase,
            sequence,
            logical_id: hex::encode(iroha_crypto::sha256(
                format!("{}:{}:{sequence}", expected.seed, phase_text(phase)).as_bytes(),
            )),
            scheduled_offset_ns,
            account_index: (ordinal % expected.accounts.len() + self.account_offset)
                % expected.accounts.len(),
        })
    }
    pub(super) fn end(&self, expected: &JournalExpectations, phase: WorkloadPhase) -> i64 {
        match phase {
            WorkloadPhase::Warmup => -expected.timing.drain_ns,
            WorkloadPhase::Measurement => expected.timing.measurement_ns,
        }
    }
    pub(super) fn within(
        &self,
        _expected: &JournalExpectations,
        phase: WorkloadPhase,
        offset: i64,
    ) -> bool {
        match phase {
            WorkloadPhase::Warmup => offset < 0,
            WorkloadPhase::Measurement => offset <= self.final_ns,
        }
    }
}

//! Admit every physical control/resource event without claiming capture authentication.

use super::*;

pub(super) struct ResourceState {
    preflight: bool,
    samples: usize,
    pending: Option<(usize, Option<i64>, i64)>,
    last_end: i64,
    finished: bool,
}
impl ResourceState {
    pub(super) fn new() -> Self {
        Self {
            preflight: false,
            samples: 0,
            pending: None,
            last_end: 0,
            finished: false,
        }
    }
}

impl Reader {
    pub(super) fn consume_control(&mut self, row: &Value, event: &str) -> Result<()> {
        match event {
            "plan" => self.plan_event(row),
            "scheduled" => {
                fields(row, &["event", "index", "plan"])?;
                ensure!(
                    self.resources.preflight && !self.clock && self.scheduled < self.records.len(),
                    "scheduled event order invalid"
                );
                equal_uint(row, "index", self.scheduled)?;
                self.records[self.scheduled]
                    .plan
                    .matches(field(row, "plan")?)?;
                self.scheduled += 1;
                Ok(())
            }
            "workload_account_preflight" => self.account_preflight(row),
            "clock_started" => {
                fields(row, &["event", "initial_offset_ns"])?;
                ensure!(
                    self.resources.preflight
                        && !self.clock
                        && self.scheduled == self.records.len()
                        && self.preflight.len() == self.expected.accounts.len(),
                    "clock before complete preflight"
                );
                equal_int(row, "initial_offset_ns", self.derived.origin)?;
                self.clock = true;
                Ok(())
            }
            "resource_preflight"
            | "resource_request"
            | "resource_observation"
            | "resource_collection_finished" => self.resource(row, event),
            "workload_postconditions_started" => {
                fields(row, &["event"])?;
                ensure!(
                    self.resources.finished
                        && !self.post_started
                        && self.active.is_none()
                        && self.records.iter().all(|r| r.accepted.is_some()
                            && r.applied.is_some()
                            && r.local_applied.is_some()),
                    "postconditions before complete transaction/resource drain"
                );
                self.post_started = true;
                Ok(())
            }
            "workload_account_postcondition" => self.account_postcondition(row),
            "request_final" => self.final_event(row),
            "collection_finished" => {
                fields(row, &["event", "passed", "failure"])?;
                ensure!(
                    self.resources.finished
                        && self.postconditions == self.expected.accounts.len()
                        && self.finals == self.records.len()
                        && self.active.is_none()
                        && field(row, "passed")?.as_bool() == Some(true)
                        && field(row, "failure")?.is_null(),
                    "collection terminal is failed or incomplete"
                );
                self.finished = true;
                Ok(())
            }
            _ => Err(eyre!("unknown or failed physical journal event")),
        }
    }
    fn plan_event(&mut self, row: &Value) -> Result<()> {
        ensure!(!self.planned, "duplicate journal plan");
        fields(
            row,
            &[
                "event",
                "schema",
                "pair_index",
                "variant",
                "seed",
                "accounts",
                "account_selection",
                "workload",
                "max_effects_per_account",
                "local_applied_required",
                "scheduled_requests",
                "warmup_ns",
                "measurement_ns",
                "drain_ns",
                "submission_lag_bound_ns",
                "preparation_lookahead",
                "preparation_concurrency",
                "preparation_ahead_ns",
                "max_submissions",
                "max_in_flight",
                "max_status_requests",
                "poll_interval_ns",
            ],
        )?;
        equal_text(row, "schema", SCHEMA)?;
        equal_uint(row, "pair_index", usize::from(self.expected.pair_index))?;
        equal_text(row, "variant", self.expected.variant.text())?;
        equal_text(row, "seed", &self.expected.seed)?;
        equal_text(row, "account_selection", SELECTION)?;
        equal_text(row, "workload", WORKLOAD)?;
        equal_uint(row, "max_effects_per_account", 1024)?;
        ensure!(
            field(row, "local_applied_required")?.as_bool() == Some(true),
            "journal requires peer-local StateApplied"
        );
        equal_uint(row, "scheduled_requests", self.records.len())?;
        let time = &self.expected.timing;
        let bounds = &self.expected.bounds;
        for (name, expected) in [
            ("warmup_ns", time.warmup_ns),
            ("measurement_ns", time.measurement_ns),
            ("drain_ns", time.drain_ns),
            ("submission_lag_bound_ns", time.submission_lag_bound_ns),
            ("preparation_ahead_ns", bounds.preparation_ahead_ns),
            ("poll_interval_ns", bounds.poll_interval_ns),
        ] {
            equal_int(row, name, expected)?;
        }
        for (name, expected) in [
            ("preparation_lookahead", bounds.preparation_lookahead),
            ("preparation_concurrency", bounds.preparation_concurrency),
            ("max_submissions", bounds.max_submissions),
            ("max_in_flight", bounds.max_in_flight),
            ("max_status_requests", bounds.max_status_requests),
        ] {
            equal_uint(row, name, expected)?;
        }
        let accounts = field(row, "accounts")?
            .as_array()
            .ok_or_else(|| eyre!("journal account pool must be array"))?;
        ensure!(
            accounts.len() == self.expected.accounts.len(),
            "journal account pool size mismatch"
        );
        for (actual, expected) in accounts.iter().zip(&self.expected.accounts) {
            fields(actual, &["authority"])?;
            equal_text(actual, "authority", &expected.authority.to_string())?;
        }
        self.planned = true;
        Ok(())
    }
    fn account_preflight(&mut self, row: &Value) -> Result<()> {
        fields(
            row,
            &[
                "event",
                "authority",
                "account_index",
                "expected_effects",
                "expected_account_sha256",
                "expected_account_frame_bytes",
            ],
        )?;
        let index = self.preflight.len();
        ensure!(
            !self.clock
                && self.scheduled == self.records.len()
                && index < self.expected.accounts.len(),
            "account preflight order invalid"
        );
        equal_uint(row, "account_index", index)?;
        equal_text(
            row,
            "authority",
            &self.expected.accounts[index].authority.to_string(),
        )?;
        equal_uint(row, "expected_effects", self.derived.effects)?;
        uint(row, "expected_account_frame_bytes", 1, 256 * 1024)?;
        self.preflight
            .push(digest(text(row, "expected_account_sha256")?)?);
        Ok(())
    }
    fn account_postcondition(&mut self, row: &Value) -> Result<()> {
        fields(
            row,
            &[
                "event",
                "authority",
                "account_index",
                "verified_effects",
                "account_sha256",
                "read_source",
            ],
        )?;
        let index = self.postconditions;
        ensure!(
            self.post_started && index < self.expected.accounts.len() && self.finals == 0,
            "account postcondition order invalid"
        );
        equal_uint(row, "account_index", index)?;
        equal_text(
            row,
            "authority",
            &self.expected.accounts[index].authority.to_string(),
        )?;
        equal_uint(row, "verified_effects", self.derived.effects)?;
        ensure!(
            digest(text(row, "account_sha256")?)? == self.preflight[index],
            "account postcondition digest mismatch"
        );
        equal_text(
            row,
            "read_source",
            "signed_find_account_by_id_after_complete_drain",
        )?;
        self.postconditions += 1;
        Ok(())
    }
    fn final_event(&mut self, row: &Value) -> Result<()> {
        fields(
            row,
            &[
                "event",
                "plan",
                "hash",
                "offer_offset_ns",
                "acknowledgment_offset_ns",
                "applied_offset_ns",
                "block_height",
                "status_attempts",
                "local_applied_offset_ns",
                "local_block_height",
                "local_status_attempts",
                "submission_finished",
                "failure",
            ],
        )?;
        ensure!(
            self.resources.finished
                && self.postconditions == self.expected.accounts.len()
                && self.finals < self.records.len(),
            "final event order invalid"
        );
        let record = &mut self.records[self.finals];
        record.plan.matches(field(row, "plan")?)?;
        equal_text(
            row,
            "hash",
            record
                .hash
                .as_deref()
                .ok_or_else(|| eyre!("final without retained request"))?,
        )?;
        let offer = record.offered.ok_or_else(|| eyre!("final without offer"))?;
        let ack = record
            .accepted
            .ok_or_else(|| eyre!("final without accepted event"))?;
        let (applied, height) = record
            .applied
            .ok_or_else(|| eyre!("final without StateApplied event"))?;
        equal_int(row, "offer_offset_ns", offer)?;
        equal_int(row, "acknowledgment_offset_ns", ack)?;
        equal_int(row, "applied_offset_ns", applied)?;
        ensure!(
            uint(row, "block_height", 1, u64::MAX)? == height,
            "final height differs from StateApplied event"
        );
        equal_uint(row, "status_attempts", record.attempts)?;
        let (local_applied, local_height) = record
            .local_applied
            .ok_or_else(|| eyre!("final without local StateApplied event"))?;
        equal_int(row, "local_applied_offset_ns", local_applied)?;
        ensure!(
            local_height == height && uint(row, "local_block_height", 1, u64::MAX)? == local_height,
            "final local height differs from exact StateApplied height"
        );
        equal_uint(row, "local_status_attempts", record.local_attempts)?;
        ensure!(
            field(row, "submission_finished")?.as_bool() == Some(true)
                && field(row, "failure")?.is_null(),
            "final request failed or unfinished"
        );
        record.final_row = Some(JournalObservation {
            sequence: record.plan.sequence,
            scheduled_offset_ns: record.plan.scheduled_offset_ns,
            offer_offset_ns: offer,
            acknowledgment_offset_ns: ack,
            applied_offset_ns: applied,
            block_height: height,
            status_attempts: record.attempts,
        });
        self.finals += 1;
        Ok(())
    }
    fn sampling(&self, value: &Value) -> Result<()> {
        fields(
            value,
            &[
                "interval_ns",
                "response_deadline_ns",
                "max_start_lag_ns",
                "first_offset_ns",
                "final_offset_ns",
                "sample_count",
            ],
        )?;
        equal_int(value, "interval_ns", self.expected.sampling.interval_ns)?;
        equal_int(
            value,
            "response_deadline_ns",
            self.expected.sampling.response_deadline_ns,
        )?;
        equal_int(
            value,
            "max_start_lag_ns",
            self.expected.sampling.max_start_lag_ns,
        )?;
        equal_int(value, "first_offset_ns", 0)?;
        equal_int(value, "final_offset_ns", self.derived.final_ns)?;
        equal_uint(value, "sample_count", self.derived.samples)
    }
    fn resource(&mut self, row: &Value, event: &str) -> Result<()> {
        match event {
            "resource_preflight" => {
                fields(
                    row,
                    &["event", "sequence", "outcome", "manifest", "sampling"],
                )?;
                ensure!(
                    !self.resources.preflight && !self.clock && self.scheduled == 0,
                    "resource preflight order invalid"
                );
                equal_uint(row, "sequence", 0)?;
                equal_text(row, "outcome", "complete")?;
                manifest(field(row, "manifest")?, "preflight", 0)?;
                self.sampling(field(row, "sampling")?)?;
                self.resources.preflight = true;
            }
            "resource_request" => {
                ensure!(
                    self.clock && !self.resources.finished && self.resources.pending.is_none(),
                    "resource request order invalid"
                );
                let finish = self.resources.samples == self.derived.samples;
                fields(
                    row,
                    if finish {
                        &["event", "kind", "sequence", "start_offset_ns"]
                    } else {
                        &[
                            "event",
                            "kind",
                            "sequence",
                            "scheduled_offset_ns",
                            "start_offset_ns",
                        ]
                    },
                )?;
                equal_text(row, "kind", if finish { "finish" } else { "sample" })?;
                let sequence = self.resources.samples + 1;
                equal_uint(row, "sequence", sequence)?;
                let start = integer(row, "start_offset_ns")?;
                ensure!(start >= self.resources.last_end, "resource clock reordered");
                let scheduled = if finish {
                    None
                } else {
                    let offset = i64::try_from(self.resources.samples)?
                        .checked_mul(self.expected.sampling.interval_ns)
                        .ok_or_else(|| eyre!("resource schedule overflow"))?;
                    equal_int(row, "scheduled_offset_ns", offset)?;
                    ensure!(
                        (offset..=offset + self.expected.sampling.max_start_lag_ns)
                            .contains(&start),
                        "resource sample outside fixed start bound"
                    );
                    Some(offset)
                };
                self.resources.pending = Some((sequence, scheduled, start));
            }
            "resource_observation" => {
                fields(
                    row,
                    &[
                        "event",
                        "sequence",
                        "scheduled_offset_ns",
                        "start_offset_ns",
                        "end_offset_ns",
                        "outcome",
                        "manifest",
                    ],
                )?;
                let (sequence, scheduled, start) = self
                    .resources
                    .pending
                    .ok_or_else(|| eyre!("resource observation without request"))?;
                let scheduled =
                    scheduled.ok_or_else(|| eyre!("sample observation for finish request"))?;
                equal_uint(row, "sequence", sequence)?;
                equal_int(row, "scheduled_offset_ns", scheduled)?;
                equal_int(row, "start_offset_ns", start)?;
                equal_text(row, "outcome", "complete")?;
                let end = integer(row, "end_offset_ns")?;
                let mut deadline = start
                    .checked_add(self.expected.sampling.response_deadline_ns)
                    .ok_or_else(|| eyre!("resource deadline overflow"))?;
                if scheduled == self.derived.final_ns {
                    deadline = deadline
                        .min(self.derived.final_ns + self.expected.sampling.response_deadline_ns);
                }
                ensure!(
                    end >= start && end < deadline,
                    "resource sample deadline exceeded"
                );
                manifest(field(row, "manifest")?, "sample", sequence)?;
                self.resources.samples += 1;
                self.resources.last_end = end;
                self.resources.pending = None;
            }
            "resource_collection_finished" => {
                fields(
                    row,
                    &[
                        "event",
                        "sequence",
                        "start_offset_ns",
                        "end_offset_ns",
                        "sampling",
                    ],
                )?;
                let (sequence, scheduled, start) = self
                    .resources
                    .pending
                    .ok_or_else(|| eyre!("resource finish without request"))?;
                ensure!(
                    scheduled.is_none()
                        && self.resources.samples == self.derived.samples
                        && !self.resources.finished,
                    "resource finish order invalid"
                );
                equal_uint(row, "sequence", sequence)?;
                equal_int(row, "start_offset_ns", start)?;
                self.sampling(field(row, "sampling")?)?;
                let end = integer(row, "end_offset_ns")?;
                ensure!(
                    end >= start
                        && start
                            .checked_add(self.expected.sampling.response_deadline_ns)
                            .is_some_and(|deadline| end < deadline),
                    "resource finish deadline exceeded"
                );
                self.resources.pending = None;
                self.resources.finished = true;
            }
            _ => unreachable!(),
        }
        Ok(())
    }
}
fn manifest(value: &Value, kind: &str, sequence: usize) -> Result<()> {
    // Only the reference's physical schema/identity is checked here. No file is opened, and no
    // capture digest, process identity, availability, RSS or storage claim is authenticated.
    fields(value, &["name", "sha256", "bytes"])?;
    equal_text(value, "name", &format!("{kind}-{sequence:010}.json"))?;
    digest(text(value, "sha256")?)?;
    uint(value, "bytes", 1, 1024 * 1024)?;
    Ok(())
}

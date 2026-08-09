//! Supervised lifecycle for finalized SoraFS billing and hedge-intent projection.
//!
//! Deterministic accounting remains in `sorafs_node`. This module only binds
//! public config-pinned policy to runtime-only production adapters, schedules
//! bounded reconciliation, and exposes payload-free health. Every data API
//! call is fenced by a live authenticated finalized-head read and the same
//! bounded freshness policy as daemon readiness; acknowledgement commits
//! recheck that exact head immediately before durable mutation. The runtime
//! deliberately has no automatic hedge-execution adapter or execution timer.

use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, bail};
use iroha_config::parameters::{actual::SorafsHedgingBillingRuntime, is_production_runtime_handle};
use iroha_data_model::ChainId;
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use sorafs_manifest::hedging::signed::HedgingFeedTrustPolicyV1;
use sorafs_node::hedging_billing_service::{
    BillingPublishedStatementRequestV1, BillingPublishedStatementV1,
    BillingStatementAcknowledgementAuthority, BillingStatementAcknowledgementRequestV1,
    BillingStatementAcknowledgementResponseV1, BillingStatementDeliveryStatusV1,
    BillingStatementListRequestV1, BillingStatementPageV1, BillingStatementPublisher,
    BillingStatementRuntimeSigner, HEDGING_BILLING_MAX_DELIVERY_WORK_ITEMS_V1, HedgeIntentPageV1,
    HedgingBillingDaemonMetricsV1, HedgingBillingDaemonStatusV1, HedgingBillingEpochWitnessStore,
    HedgingBillingExposurePageV1, HedgingBillingExternalError, HedgingBillingFinalizedCursorV1,
    HedgingBillingFinalizedQuery, HedgingBillingJournalVerifier, HedgingBillingProjectionAnchorV1,
    HedgingBillingProjectionPageRequestV1, HedgingBillingReconciliationStatusV1,
    HedgingBillingRuntimeApiErrorV1, HedgingBillingRuntimeApiV1,
    HedgingBillingRuntimeProviderQualificationV1, HedgingBillingService,
    HedgingBillingServiceError, HedgingBillingServicePolicyV1,
    QualifiedHedgingBillingRuntimeProviderV1,
};

const SHUTDOWN_WAIT: Duration = Duration::from_secs(2);
const READINESS_STALE_TICK_MULTIPLIER_V1: u32 = 3;

/// Runtime-only dependencies for the committed hedging/billing worker.
#[derive(Clone)]
pub(crate) struct HedgingBillingRuntimeDependenciesV1 {
    pub(crate) finalized_query: Arc<dyn HedgingBillingFinalizedQuery>,
    pub(crate) journal_verifier: Arc<dyn HedgingBillingJournalVerifier>,
    pub(crate) statement_signer: Arc<dyn BillingStatementRuntimeSigner>,
    pub(crate) statement_publisher: Arc<dyn BillingStatementPublisher>,
    pub(crate) acknowledgement_authority: Arc<dyn BillingStatementAcknowledgementAuthority>,
    pub(crate) epoch_witness_store: Arc<dyn HedgingBillingEpochWitnessStore>,
}

impl fmt::Debug for HedgingBillingRuntimeDependenciesV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HedgingBillingRuntimeDependenciesV1")
            .field("runtime_only_adapters", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl HedgingBillingRuntimeDependenciesV1 {
    /// Require every runtime-only production adapter before daemon assembly.
    pub(crate) fn require(
        finalized_query: Option<Arc<dyn HedgingBillingFinalizedQuery>>,
        journal_verifier: Option<Arc<dyn HedgingBillingJournalVerifier>>,
        statement_signer: Option<Arc<dyn BillingStatementRuntimeSigner>>,
        statement_publisher: Option<Arc<dyn BillingStatementPublisher>>,
        acknowledgement_authority: Option<Arc<dyn BillingStatementAcknowledgementAuthority>>,
        epoch_witness_store: Option<Arc<dyn HedgingBillingEpochWitnessStore>>,
    ) -> Result<Self> {
        Ok(Self {
            finalized_query: finalized_query
                .ok_or_else(|| eyre::eyre!("missing finalized billing query adapter"))?,
            journal_verifier: journal_verifier
                .ok_or_else(|| eyre::eyre!("missing consensus billing journal verifier"))?,
            statement_signer: statement_signer
                .ok_or_else(|| eyre::eyre!("missing billing statement HSM/KMS signer"))?,
            statement_publisher: statement_publisher
                .ok_or_else(|| eyre::eyre!("missing immutable billing statement publisher"))?,
            acknowledgement_authority: acknowledgement_authority
                .ok_or_else(|| eyre::eyre!("missing billing acknowledgement authority"))?,
            epoch_witness_store: epoch_witness_store
                .ok_or_else(|| eyre::eyre!("missing sealed billing epoch witness store"))?,
        })
    }
}

#[derive(Debug, Default)]
struct HedgingBillingDaemonCounters {
    successful_ticks: AtomicU64,
    failed_ticks: AtomicU64,
    panicked_ticks: AtomicU64,
    finalized_pages_applied: AtomicU64,
    finalized_events_applied: AtomicU64,
    period_closes_applied: AtomicU64,
    statements_signed: AtomicU64,
    statements_published: AtomicU64,
    publications_reconciled: AtomicU64,
    acknowledgements_reconciled: AtomicU64,
}

#[derive(Debug, Default)]
struct TickOutcome {
    finalized_pages_applied: u64,
    finalized_events_applied: u64,
    period_closes_applied: u64,
    statements_signed: u64,
    statements_published: u64,
    publications_reconciled: u64,
    acknowledgements_reconciled: u64,
}

struct HedgingBillingRuntimeInnerV1 {
    config: SorafsHedgingBillingRuntime,
    policy: HedgingBillingServicePolicyV1,
    service: Arc<HedgingBillingService>,
    dependencies: HedgingBillingRuntimeDependenciesV1,
}

impl fmt::Debug for HedgingBillingRuntimeInnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HedgingBillingRuntimeInnerV1")
            .field("chain_id", &self.policy.chain_id)
            .field("policy_revision", &self.policy.revision)
            .field("runtime_only_adapters", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

/// Cloneable status/metrics handle retained by `irohad`.
#[derive(Clone)]
pub struct HedgingBillingRuntimeHandleV1 {
    inner: Arc<HedgingBillingRuntimeInnerV1>,
    counters: Arc<HedgingBillingDaemonCounters>,
    external_dependencies_healthy: Arc<AtomicBool>,
    last_tick_healthy: Arc<AtomicBool>,
    last_successful_tick: Arc<Mutex<Option<Instant>>>,
    latest_finalized_head: Arc<Mutex<Option<HedgingBillingFinalizedCursorV1>>>,
    tick_lock: Arc<Mutex<()>>,
    delivery_scan_sequence: Arc<AtomicU64>,
}

impl fmt::Debug for HedgingBillingRuntimeHandleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HedgingBillingRuntimeHandleV1")
            .field("inner", &self.inner)
            .field("runtime_state", &"[PAYLOAD-FREE]")
            .finish_non_exhaustive()
    }
}

impl HedgingBillingRuntimeHandleV1 {
    fn last_successful_tick_is_fresh(&self) -> bool {
        let freshness_guard = match self.last_successful_tick.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        freshness_guard.as_ref().is_some_and(|instant| {
            instant.elapsed()
                <= self
                    .inner
                    .config
                    .poll_interval
                    .checked_mul(READINESS_STALE_TICK_MULTIPLIER_V1)
                    .unwrap_or(Duration::MAX)
        })
    }

    /// Return payload-free health and readiness without performing work.
    ///
    /// # Errors
    ///
    /// Fails only when the deterministic service state cannot be inspected.
    pub fn status(&self) -> Result<HedgingBillingDaemonStatusV1> {
        let (anchor, service) = self
            .inner
            .service
            .api_anchored_service_status()
            .wrap_err("inspect hedging/billing service state")?;
        let live = self.counters.successful_ticks.load(Ordering::Acquire) != 0;
        let external_dependencies_healthy =
            self.external_dependencies_healthy.load(Ordering::Acquire);
        let last_tick_healthy = self.last_tick_healthy.load(Ordering::Acquire);
        let last_tick_fresh = self.last_successful_tick_is_fresh();
        let finalized_head = {
            let finalized_head_guard = match self.latest_finalized_head.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            *finalized_head_guard
        };
        let finalized_head_height = finalized_head.map_or(0, |cursor| cursor.height);
        let finalized_lag_blocks = finalized_head_height.saturating_sub(service.finalized_height);
        let finalized_projection_ready = finalized_head.is_some_and(|head| {
            projection_is_fresh_at_head(
                &anchor,
                service.finalized_height,
                head,
                self.inner.config.max_finalized_lag_blocks,
            )
        });
        let automatic_hedge_execution_enabled = false;
        let ready = live
            && external_dependencies_healthy
            && last_tick_healthy
            && last_tick_fresh
            && finalized_projection_ready
            && service.dead_letter == 0;
        Ok(HedgingBillingDaemonStatusV1 {
            anchor,
            service,
            live,
            external_dependencies_healthy,
            last_tick_healthy,
            last_tick_fresh,
            finalized_projection_ready,
            finalized_head_height,
            finalized_lag_blocks,
            automatic_hedge_execution_enabled,
            ready,
        })
    }

    /// Return payload-free bounded worker counters.
    #[must_use]
    pub fn metrics(&self) -> HedgingBillingDaemonMetricsV1 {
        HedgingBillingDaemonMetricsV1 {
            successful_ticks: self.counters.successful_ticks.load(Ordering::Relaxed),
            failed_ticks: self.counters.failed_ticks.load(Ordering::Relaxed),
            panicked_ticks: self.counters.panicked_ticks.load(Ordering::Relaxed),
            finalized_pages_applied: self
                .counters
                .finalized_pages_applied
                .load(Ordering::Relaxed),
            finalized_events_applied: self
                .counters
                .finalized_events_applied
                .load(Ordering::Relaxed),
            period_closes_applied: self.counters.period_closes_applied.load(Ordering::Relaxed),
            statements_signed: self.counters.statements_signed.load(Ordering::Relaxed),
            statements_published: self.counters.statements_published.load(Ordering::Relaxed),
            publications_reconciled: self
                .counters
                .publications_reconciled
                .load(Ordering::Relaxed),
            acknowledgements_reconciled: self
                .counters
                .acknowledgements_reconciled
                .load(Ordering::Relaxed),
        }
    }

    fn mark_tick_failed(&self, error: eyre::Report) -> eyre::Report {
        self.external_dependencies_healthy
            .store(false, Ordering::Release);
        self.last_tick_healthy.store(false, Ordering::Release);
        self.counters.failed_ticks.fetch_add(1, Ordering::Relaxed);
        error
    }

    fn probe_finalized_head_for_api(
        &self,
    ) -> std::result::Result<HedgingBillingFinalizedCursorV1, HedgingBillingRuntimeApiErrorV1> {
        probe_finalized_query(&self.inner.config, &self.inner.dependencies).map_err(|_| {
            self.external_dependencies_healthy
                .store(false, Ordering::Release);
            HedgingBillingRuntimeApiErrorV1::Unavailable
        })
    }

    fn require_fresh_projection(
        &self,
    ) -> std::result::Result<
        (
            std::sync::MutexGuard<'_, ()>,
            HedgingBillingFinalizedCursorV1,
        ),
        HedgingBillingRuntimeApiErrorV1,
    > {
        let tick_guard = self
            .tick_lock
            .lock()
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::Unavailable)?;
        if self.counters.successful_ticks.load(Ordering::Acquire) == 0
            || !self.external_dependencies_healthy.load(Ordering::Acquire)
            || !self.last_tick_healthy.load(Ordering::Acquire)
            || !self.last_successful_tick_is_fresh()
        {
            return Err(HedgingBillingRuntimeApiErrorV1::Unavailable);
        }
        let (anchor, service) = self.inner.service.api_anchored_service_status()?;
        if service.dead_letter != 0 {
            return Err(HedgingBillingRuntimeApiErrorV1::Unavailable);
        }
        let finalized_head = self.probe_finalized_head_for_api()?;
        let previous_head = self
            .latest_finalized_head
            .lock()
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::Unavailable)?
            .ok_or(HedgingBillingRuntimeApiErrorV1::Unavailable)?;
        if !finalized_head_extends(previous_head, finalized_head) {
            self.external_dependencies_healthy
                .store(false, Ordering::Release);
            return Err(HedgingBillingRuntimeApiErrorV1::Unavailable);
        }
        if !projection_is_fresh_at_head(
            &anchor,
            service.finalized_height,
            finalized_head,
            self.inner.config.max_finalized_lag_blocks,
        ) {
            return Err(HedgingBillingRuntimeApiErrorV1::Unavailable);
        }
        Ok((tick_guard, finalized_head))
    }

    fn recheck_fresh_projection_head(
        &self,
        expected_head: HedgingBillingFinalizedCursorV1,
    ) -> std::result::Result<(), HedgingBillingRuntimeApiErrorV1> {
        if self.counters.successful_ticks.load(Ordering::Acquire) == 0
            || !self.external_dependencies_healthy.load(Ordering::Acquire)
            || !self.last_tick_healthy.load(Ordering::Acquire)
            || !self.last_successful_tick_is_fresh()
        {
            return Err(HedgingBillingRuntimeApiErrorV1::Unavailable);
        }
        if self.probe_finalized_head_for_api()? != expected_head {
            return Err(HedgingBillingRuntimeApiErrorV1::Unavailable);
        }
        Ok(())
    }

    fn with_fresh_projection<T>(
        &self,
        operation: impl FnOnce(
            &HedgingBillingService,
            &mut dyn FnMut() -> std::result::Result<(), HedgingBillingServiceError>,
        ) -> std::result::Result<T, HedgingBillingRuntimeApiErrorV1>,
    ) -> std::result::Result<T, HedgingBillingRuntimeApiErrorV1> {
        let (_tick_guard, finalized_head) = self.require_fresh_projection()?;
        let mut pre_commit_fence = || {
            self.recheck_fresh_projection_head(finalized_head)
                .map_err(|_| {
                    HedgingBillingServiceError::External(HedgingBillingExternalError::Unavailable)
                })
        };
        let response = operation(self.inner.service.as_ref(), &mut pre_commit_fence)?;
        self.recheck_fresh_projection_head(finalized_head)?;
        Ok(response)
    }

    fn verify_projection_at_or_before_head(
        &self,
        finalized_head: HedgingBillingFinalizedCursorV1,
        operation: &'static str,
    ) -> Result<()> {
        let projection_anchor = self
            .inner
            .service
            .api_projection_anchor()
            .map_err(|error| eyre::eyre!("{operation}: {error}"))?;
        if !projection_at_or_before_head(projection_anchor.finalized_cursor, finalized_head) {
            bail!("finalized billing query head conflicts with durable projector cursor");
        }
        Ok(())
    }

    fn record_successful_tick(&self, outcome: &TickOutcome) {
        self.last_tick_healthy.store(true, Ordering::Release);
        let mut freshness_guard = match self.last_successful_tick.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *freshness_guard = Some(Instant::now());
        self.counters
            .successful_ticks
            .fetch_add(1, Ordering::Relaxed);
        self.counters
            .finalized_pages_applied
            .fetch_add(outcome.finalized_pages_applied, Ordering::Relaxed);
        self.counters
            .finalized_events_applied
            .fetch_add(outcome.finalized_events_applied, Ordering::Relaxed);
        self.counters
            .period_closes_applied
            .fetch_add(outcome.period_closes_applied, Ordering::Relaxed);
        self.counters
            .statements_signed
            .fetch_add(outcome.statements_signed, Ordering::Relaxed);
        self.counters
            .statements_published
            .fetch_add(outcome.statements_published, Ordering::Relaxed);
        self.counters
            .publications_reconciled
            .fetch_add(outcome.publications_reconciled, Ordering::Relaxed);
        self.counters
            .acknowledgements_reconciled
            .fetch_add(outcome.acknowledgements_reconciled, Ordering::Relaxed);
    }

    fn reconcile_once(&self) -> Result<()> {
        let _tick_guard = match self.tick_lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let finalized_head = probe_dependencies(
            &self.inner.config,
            &self.inner.policy,
            &self.inner.dependencies,
        )
        .map_err(|error| self.mark_tick_failed(error))?;
        self.verify_projection_at_or_before_head(
            finalized_head,
            "inspect finalized billing projection anchor",
        )
        .map_err(|error| self.mark_tick_failed(error))?;
        {
            let previous_head_guard = match self.latest_finalized_head.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if previous_head_guard
                .is_some_and(|previous| !finalized_head_extends(previous, finalized_head))
            {
                return Err(self.mark_tick_failed(eyre::eyre!(
                    "finalized billing query head regressed or equivocated"
                )));
            }
        }
        self.external_dependencies_healthy
            .store(true, Ordering::Release);
        let delivery_scan_sequence = self.delivery_scan_sequence.fetch_add(1, Ordering::Relaxed);
        let outcome = reconcile_once(&self.inner, delivery_scan_sequence, finalized_head)
            .map_err(|error| self.mark_tick_failed(error))?;
        self.verify_projection_at_or_before_head(
            finalized_head,
            "inspect finalized billing projection anchor after reconciliation",
        )
        .map_err(|error| self.mark_tick_failed(error))?;
        {
            let mut finalized_head_guard = match self.latest_finalized_head.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if finalized_head_guard
                .is_some_and(|previous| !finalized_head_extends(previous, finalized_head))
            {
                return Err(self.mark_tick_failed(eyre::eyre!(
                    "finalized billing query head changed concurrently"
                )));
            }
            *finalized_head_guard = Some(finalized_head);
        }
        self.record_successful_tick(&outcome);
        Ok(())
    }
}

impl HedgingBillingRuntimeApiV1 for HedgingBillingRuntimeHandleV1 {
    fn projection_anchor(
        &self,
    ) -> std::result::Result<HedgingBillingProjectionAnchorV1, HedgingBillingRuntimeApiErrorV1>
    {
        self.with_fresh_projection(|service, _| service.api_projection_anchor())
    }

    fn list_statements(
        &self,
        request: &BillingStatementListRequestV1,
    ) -> std::result::Result<BillingStatementPageV1, HedgingBillingRuntimeApiErrorV1> {
        self.with_fresh_projection(|service, _| service.api_list_statements(request))
    }

    fn published_statement(
        &self,
        request: &BillingPublishedStatementRequestV1,
    ) -> std::result::Result<BillingPublishedStatementV1, HedgingBillingRuntimeApiErrorV1> {
        self.with_fresh_projection(|service, _| service.api_published_statement(request))
    }

    fn acknowledge_statement(
        &self,
        request: &BillingStatementAcknowledgementRequestV1,
        server_time_unix: u64,
    ) -> std::result::Result<
        BillingStatementAcknowledgementResponseV1,
        HedgingBillingRuntimeApiErrorV1,
    > {
        self.with_fresh_projection(|service, pre_commit_fence| {
            service.api_acknowledge_statement_with_precommit_fence(
                request,
                server_time_unix,
                pre_commit_fence,
            )
        })
    }

    fn exposure_page(
        &self,
        request: &HedgingBillingProjectionPageRequestV1,
    ) -> std::result::Result<HedgingBillingExposurePageV1, HedgingBillingRuntimeApiErrorV1> {
        self.with_fresh_projection(|service, _| service.api_exposure_page(request))
    }

    fn hedge_intent_page(
        &self,
        request: &HedgingBillingProjectionPageRequestV1,
    ) -> std::result::Result<HedgeIntentPageV1, HedgingBillingRuntimeApiErrorV1> {
        self.with_fresh_projection(|service, _| service.api_hedge_intent_page(request))
    }

    fn daemon_status(
        &self,
    ) -> std::result::Result<HedgingBillingDaemonStatusV1, HedgingBillingRuntimeApiErrorV1> {
        HedgingBillingRuntimeHandleV1::status(self)
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::Unavailable)
    }

    fn daemon_metrics(&self) -> HedgingBillingDaemonMetricsV1 {
        HedgingBillingRuntimeHandleV1::metrics(self)
    }

    fn reconciliation_status(
        &self,
    ) -> std::result::Result<HedgingBillingReconciliationStatusV1, HedgingBillingRuntimeApiErrorV1>
    {
        self.with_fresh_projection(|service, _| {
            let (anchor, service) = service.api_anchored_service_status()?;
            let metrics = HedgingBillingRuntimeHandleV1::metrics(self);
            let pending_delivery_operations = service
                .ready_for_signing
                .checked_add(service.signing)
                .and_then(|value| value.checked_add(service.ready_for_publication))
                .and_then(|value| value.checked_add(service.publication_ambiguous))
                .and_then(|value| value.checked_add(service.published))
                .ok_or(HedgingBillingRuntimeApiErrorV1::ResourceExhausted)?;
            Ok(HedgingBillingReconciliationStatusV1 {
                anchor,
                last_tick_healthy: self.last_tick_healthy.load(Ordering::Acquire),
                successful_ticks: metrics.successful_ticks,
                failed_ticks: metrics.failed_ticks,
                pending_delivery_operations,
            })
        })
    }
}

/// Assemble and start the committed hedging/billing runtime.
///
/// Missing, test-marked, unready, identity-substituted, or period-close
/// incapable adapters fail startup before the private checkpoint is opened.
pub(crate) fn start(
    config: SorafsHedgingBillingRuntime,
    chain_id: &ChainId,
    policy: HedgingBillingServicePolicyV1,
    feed_policy: &Arc<HedgingFeedTrustPolicyV1>,
    dependencies: HedgingBillingRuntimeDependenciesV1,
    shutdown_signal: ShutdownSignal,
) -> Result<(HedgingBillingRuntimeHandleV1, Child)> {
    let poll_interval = config.poll_interval;
    let handle = assemble(config, chain_id, policy, feed_policy, dependencies)?;
    record_status_metrics(&handle);

    let worker = handle.clone();
    let task = tokio::task::spawn(async move {
        let mut interval = tokio::time::interval(poll_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let tick = worker.clone();
                    match tokio::task::spawn_blocking(move || tick.reconcile_once()).await {
                        Ok(Ok(())) => {
                            record_tick_metric("success");
                        }
                        Ok(Err(error)) => {
                            record_tick_metric("failure");
                            iroha_logger::warn!(
                                error = %error,
                                "committed SoraFS hedging/billing reconciliation failed"
                            );
                        }
                        Err(error) => {
                            worker
                                .external_dependencies_healthy
                                .store(false, Ordering::Release);
                            worker.last_tick_healthy.store(false, Ordering::Release);
                            worker
                                .counters
                                .panicked_ticks
                                .fetch_add(1, Ordering::Relaxed);
                            record_tick_metric("panic");
                            iroha_logger::error!(
                                cancelled = error.is_cancelled(),
                                panicked = error.is_panic(),
                                "committed SoraFS hedging/billing worker task failed"
                            );
                        }
                    }
                    record_status_metrics(&worker);
                }
                () = shutdown_signal.receive() => {
                    iroha_logger::debug!(
                        "committed SoraFS hedging/billing runtime is being shut down"
                    );
                    break;
                }
                else => break,
            }
        }
    });
    Ok((handle, Child::new(task, OnShutdown::Wait(SHUTDOWN_WAIT))))
}

fn assemble(
    config: SorafsHedgingBillingRuntime,
    chain_id: &ChainId,
    policy: HedgingBillingServicePolicyV1,
    feed_policy: &Arc<HedgingFeedTrustPolicyV1>,
    dependencies: HedgingBillingRuntimeDependenciesV1,
) -> Result<HedgingBillingRuntimeHandleV1> {
    validate_actual_config(&config)?;
    policy
        .validate()
        .wrap_err("validate configured hedging/billing service policy")?;
    if &policy.chain_id != chain_id {
        bail!("hedging/billing service policy chain identity does not match iroha_config");
    }
    if policy
        .canonical_digest()
        .wrap_err("derive hedging/billing service policy digest")?
        != config.service_policy_digest
    {
        bail!("hedging/billing service policy digest does not match iroha_config");
    }
    if feed_policy
        .canonical_digest()
        .wrap_err("derive hedging feed trust-policy digest")?
        != policy.feed_trust_policy_digest
    {
        bail!("hedging/billing feed trust-policy digest does not match service policy");
    }
    if config.epoch_witness_store_handle != policy.epoch_witness_store_handle {
        bail!("sealed billing epoch store handle does not match service policy");
    }
    let dependencies = qualify_dependencies(&config, dependencies)?;
    let startup_finalized_head = probe_dependencies(&config, &policy, &dependencies)?;

    let service = Arc::new(
        HedgingBillingService::new(
            &config.state_dir,
            policy.clone(),
            feed_policy.as_ref().clone(),
            Arc::clone(&dependencies.journal_verifier),
            Arc::clone(&dependencies.statement_publisher),
            Arc::clone(&dependencies.acknowledgement_authority),
            Arc::clone(&dependencies.epoch_witness_store),
        )
        .wrap_err("open committed hedging/billing service")?,
    );
    let projection_anchor = service
        .api_projection_anchor()
        .wrap_err("inspect recovered hedging/billing projection anchor")?;
    if !projection_at_or_before_head(projection_anchor.finalized_cursor, startup_finalized_head) {
        bail!("finalized billing query head conflicts with recovered projector cursor");
    }
    Ok(HedgingBillingRuntimeHandleV1 {
        inner: Arc::new(HedgingBillingRuntimeInnerV1 {
            config,
            policy,
            service,
            dependencies,
        }),
        counters: Arc::new(HedgingBillingDaemonCounters::default()),
        external_dependencies_healthy: Arc::new(AtomicBool::new(true)),
        last_tick_healthy: Arc::new(AtomicBool::new(false)),
        last_successful_tick: Arc::new(Mutex::new(None)),
        latest_finalized_head: Arc::new(Mutex::new(Some(startup_finalized_head))),
        tick_lock: Arc::new(Mutex::new(())),
        delivery_scan_sequence: Arc::new(AtomicU64::new(0)),
    })
}

fn qualify_dependencies(
    config: &SorafsHedgingBillingRuntime,
    dependencies: HedgingBillingRuntimeDependenciesV1,
) -> Result<HedgingBillingRuntimeDependenciesV1> {
    let HedgingBillingRuntimeDependenciesV1 {
        finalized_query,
        journal_verifier,
        statement_signer,
        statement_publisher,
        acknowledgement_authority,
        epoch_witness_store,
    } = dependencies;
    let finalized_query: Arc<dyn HedgingBillingFinalizedQuery> = Arc::new(
        QualifiedHedgingBillingRuntimeProviderV1::try_new(
            &config.finalized_query_handle,
            HedgingBillingRuntimeProviderQualificationV1::new(
                config.finalized_query_revision,
                config.finalized_query_policy_digest,
            ),
            finalized_query,
        )
        .wrap_err("qualify finalized billing query provider")?,
    );
    let journal_verifier: Arc<dyn HedgingBillingJournalVerifier> = Arc::new(
        QualifiedHedgingBillingRuntimeProviderV1::try_new(
            &config.journal_verifier_handle,
            HedgingBillingRuntimeProviderQualificationV1::new(
                config.journal_verifier_revision,
                config.journal_verifier_policy_digest,
            ),
            journal_verifier,
        )
        .wrap_err("qualify consensus billing journal verifier")?,
    );
    let statement_signer: Arc<dyn BillingStatementRuntimeSigner> = Arc::new(
        QualifiedHedgingBillingRuntimeProviderV1::try_new(
            &config.statement_signer_handle,
            HedgingBillingRuntimeProviderQualificationV1::new(
                config.statement_signer_revision,
                config.statement_signer_policy_digest,
            ),
            statement_signer,
        )
        .wrap_err("qualify billing statement HSM/KMS signer")?,
    );
    let statement_publisher: Arc<dyn BillingStatementPublisher> = Arc::new(
        QualifiedHedgingBillingRuntimeProviderV1::try_new(
            &config.statement_publisher_handle,
            HedgingBillingRuntimeProviderQualificationV1::new(
                config.statement_publisher_revision,
                config.statement_publisher_policy_digest,
            ),
            statement_publisher,
        )
        .wrap_err("qualify immutable billing statement publisher")?,
    );
    let acknowledgement_authority: Arc<dyn BillingStatementAcknowledgementAuthority> = Arc::new(
        QualifiedHedgingBillingRuntimeProviderV1::try_new(
            &config.acknowledgement_authority_handle,
            HedgingBillingRuntimeProviderQualificationV1::new(
                config.acknowledgement_authority_revision,
                config.acknowledgement_authority_policy_digest,
            ),
            acknowledgement_authority,
        )
        .wrap_err("qualify billing acknowledgement authority")?,
    );
    let epoch_witness_store: Arc<dyn HedgingBillingEpochWitnessStore> = Arc::new(
        QualifiedHedgingBillingRuntimeProviderV1::try_new(
            &config.epoch_witness_store_handle,
            HedgingBillingRuntimeProviderQualificationV1::new(
                config.epoch_witness_store_revision,
                config.epoch_witness_store_policy_digest,
            ),
            epoch_witness_store,
        )
        .wrap_err("qualify sealed billing epoch witness store")?,
    );
    Ok(HedgingBillingRuntimeDependenciesV1 {
        finalized_query,
        journal_verifier,
        statement_signer,
        statement_publisher,
        acknowledgement_authority,
        epoch_witness_store,
    })
}

fn probe_dependencies(
    config: &SorafsHedgingBillingRuntime,
    policy: &HedgingBillingServicePolicyV1,
    dependencies: &HedgingBillingRuntimeDependenciesV1,
) -> Result<HedgingBillingFinalizedCursorV1> {
    let finalized_head = probe_finalized_query(config, dependencies)?;

    let verifier_identity = dependencies
        .journal_verifier
        .identity()
        .wrap_err("read consensus billing journal verifier identity")?;
    validate_dependency_handle(
        "consensus billing journal verifier",
        &config.journal_verifier_handle,
        &verifier_identity.handle,
    )?;
    dependencies
        .journal_verifier
        .check_readiness()
        .wrap_err("consensus billing journal verifier is not ready")?;

    let signer_identity = dependencies
        .statement_signer
        .identity()
        .wrap_err("read billing statement signer identity")?;
    validate_dependency_handle(
        "billing statement signer",
        &config.statement_signer_handle,
        &signer_identity.provider_handle,
    )?;
    if signer_identity.signer_id != policy.statement_signer.signer_id
        || signer_identity.public_key != policy.statement_signer.public_key
    {
        bail!("billing statement signer identity does not match service policy");
    }
    dependencies
        .statement_signer
        .check_readiness()
        .wrap_err("billing statement signer is not ready")?;

    let publisher_identity = dependencies
        .statement_publisher
        .identity()
        .wrap_err("read billing statement publisher identity")?;
    validate_dependency_handle(
        "billing statement publisher",
        &config.statement_publisher_handle,
        &publisher_identity.provider_handle,
    )?;
    if publisher_identity.publisher_id != policy.statement_publisher.publisher_id
        || publisher_identity.route_id != policy.statement_publisher.route_id
        || publisher_identity.public_key != policy.statement_publisher.public_key
    {
        bail!("billing statement publisher identity does not match service policy");
    }
    dependencies
        .statement_publisher
        .check_readiness()
        .wrap_err("billing statement publisher is not ready")?;

    let acknowledgement_identity = dependencies
        .acknowledgement_authority
        .identity()
        .wrap_err("read billing acknowledgement authority identity")?;
    validate_dependency_handle(
        "billing acknowledgement authority",
        &config.acknowledgement_authority_handle,
        &acknowledgement_identity.provider_handle,
    )?;
    dependencies
        .acknowledgement_authority
        .check_readiness()
        .wrap_err("billing acknowledgement authority is not ready")?;

    validate_dependency_handle(
        "sealed billing epoch witness store",
        &config.epoch_witness_store_handle,
        dependencies.epoch_witness_store.handle(),
    )?;
    dependencies
        .epoch_witness_store
        .check_readiness()
        .wrap_err("sealed billing epoch witness store is not ready")?;
    Ok(finalized_head)
}

fn probe_finalized_query(
    config: &SorafsHedgingBillingRuntime,
    dependencies: &HedgingBillingRuntimeDependenciesV1,
) -> Result<HedgingBillingFinalizedCursorV1> {
    let query_identity = dependencies
        .finalized_query
        .identity()
        .wrap_err("read finalized billing query identity")?;
    validate_dependency_handle(
        "finalized billing query",
        &config.finalized_query_handle,
        &query_identity.handle,
    )?;
    if !dependencies.finalized_query.supplies_period_closes() {
        bail!("finalized billing query does not supply typed period-close records");
    }
    dependencies
        .finalized_query
        .check_readiness()
        .wrap_err("finalized billing query is not ready")?;
    let finalized_head = dependencies
        .finalized_query
        .finalized_head()
        .map_err(|error| eyre::eyre!("query authenticated finalized billing head: {error}"))?;
    validate_finalized_head(finalized_head)?;
    Ok(finalized_head)
}

fn validate_finalized_head(cursor: HedgingBillingFinalizedCursorV1) -> Result<()> {
    if cursor.height == 0 || cursor.block_hash == [0; 32] || cursor.finalized_at_unix == 0 {
        bail!("finalized billing query returned an invalid authenticated head");
    }
    Ok(())
}

fn finalized_head_extends(
    previous: HedgingBillingFinalizedCursorV1,
    next: HedgingBillingFinalizedCursorV1,
) -> bool {
    next.finalized_at_unix >= previous.finalized_at_unix
        && (next.height > previous.height || next == previous)
}

fn projection_at_or_before_head(
    projected: Option<HedgingBillingFinalizedCursorV1>,
    head: HedgingBillingFinalizedCursorV1,
) -> bool {
    projected.is_none_or(|cursor| {
        cursor.finalized_at_unix <= head.finalized_at_unix
            && (cursor.height < head.height || cursor == head)
    })
}

fn projection_is_fresh_at_head(
    anchor: &HedgingBillingProjectionAnchorV1,
    service_finalized_height: u64,
    finalized_head: HedgingBillingFinalizedCursorV1,
    max_finalized_lag_blocks: u64,
) -> bool {
    anchor.finalized_cursor.is_some_and(|cursor| {
        cursor.height != 0
            && cursor.height == service_finalized_height
            && projection_at_or_before_head(Some(cursor), finalized_head)
            && finalized_head.height.saturating_sub(cursor.height) <= max_finalized_lag_blocks
    })
}

fn reconcile_once(
    inner: &HedgingBillingRuntimeInnerV1,
    delivery_scan_sequence: u64,
    finalized_head: HedgingBillingFinalizedCursorV1,
) -> Result<TickOutcome> {
    let scan = inner
        .service
        .reconcile_finalized_query(
            inner.dependencies.finalized_query.as_ref(),
            inner.config.max_pages_per_tick,
            finalized_head,
        )
        .wrap_err("reconcile finalized billing journal pages")?;
    let mut outcome = TickOutcome {
        finalized_pages_applied: u64::from(scan.pages_applied),
        finalized_events_applied: scan.events_applied,
        ..TickOutcome::default()
    };

    let mut next_period_end_unix = inner
        .service
        .status()
        .wrap_err("inspect next finalized billing boundary")?
        .next_period_end_unix;
    for _ in 0..inner.config.max_period_closes_per_tick {
        let position = inner
            .service
            .query_position()
            .wrap_err("inspect finalized billing query position")?;
        let Some(close) = inner
            .dependencies
            .finalized_query
            .query_finalized_period_close(next_period_end_unix, position)
            .wrap_err("query next finalized billing period close")?
        else {
            break;
        };
        if !projection_at_or_before_head(
            Some(close.journal_commitment.finalized_cursor),
            finalized_head,
        ) {
            bail!("finalized billing period close exceeds the authenticated query head");
        }
        inner
            .service
            .finalize_next_period(&close)
            .wrap_err("finalize committed billing period")?;
        outcome.period_closes_applied = outcome.period_closes_applied.saturating_add(1);
        next_period_end_unix = next_period_end_unix
            .checked_add(inner.policy.billing_period_secs)
            .ok_or_else(|| eyre::eyre!("next finalized billing period boundary overflow"))?;
    }

    // The service filters and copies at most the governed per-tick bound while
    // holding its state lock. It never clones the retained statement inventory.
    // Newly advanced records move to their next stage on the following tick.
    let projections = inner
        .service
        .pending_statement_delivery_projections_rotated(
            inner.config.max_delivery_operations_per_tick,
            delivery_scan_sequence,
        )
        .wrap_err("inspect billing statement delivery state")?;
    for projection in projections {
        match projection.status {
            BillingStatementDeliveryStatusV1::ReadyForSigning => {
                inner
                    .service
                    .sign_statement(
                        projection.statement_id,
                        inner.dependencies.statement_signer.as_ref(),
                    )
                    .wrap_err("sign next governed billing statement")?;
                outcome.statements_signed = outcome.statements_signed.saturating_add(1);
            }
            BillingStatementDeliveryStatusV1::ReadyForPublication => {
                inner
                    .service
                    .publish_statement(projection.statement_id)
                    .wrap_err("publish next governed billing statement")?;
                outcome.statements_published = outcome.statements_published.saturating_add(1);
            }
            BillingStatementDeliveryStatusV1::PublicationAmbiguous => {
                let _ = inner
                    .service
                    .reconcile_ambiguous_publication(projection.statement_id)
                    .wrap_err("reconcile ambiguous billing statement publication")?;
                outcome.publications_reconciled = outcome.publications_reconciled.saturating_add(1);
            }
            BillingStatementDeliveryStatusV1::Published => {
                if inner
                    .service
                    .reconcile_acknowledgement(projection.statement_id)
                    .wrap_err("reconcile billing statement acknowledgement")?
                    .is_some()
                {
                    outcome.acknowledgements_reconciled =
                        outcome.acknowledgements_reconciled.saturating_add(1);
                }
            }
            BillingStatementDeliveryStatusV1::Signing
            | BillingStatementDeliveryStatusV1::Acknowledged
            | BillingStatementDeliveryStatusV1::DeadLetter => {
                unreachable!("terminal and in-flight projections were filtered")
            }
        }
    }
    Ok(outcome)
}

fn validate_actual_config(config: &SorafsHedgingBillingRuntime) -> Result<()> {
    if !absolute_leaf(&config.state_dir)
        || !absolute_leaf(&config.service_policy_path)
        || config.service_policy_digest == [0; 32]
        || config.poll_interval < Duration::from_millis(100)
        || config.poll_interval > Duration::from_secs(60)
        || config.max_pages_per_tick == 0
        || config.max_pages_per_tick > 4_096
        || config.max_period_closes_per_tick == 0
        || config.max_period_closes_per_tick > 4_096
        || config.max_delivery_operations_per_tick == 0
        || config.max_delivery_operations_per_tick > HEDGING_BILLING_MAX_DELIVERY_WORK_ITEMS_V1
        || config.max_finalized_lag_blocks == 0
        || config.max_finalized_lag_blocks > 10_000
    {
        bail!("committed hedging/billing runtime configuration is invalid");
    }
    for handle in [
        &config.finalized_query_handle,
        &config.journal_verifier_handle,
        &config.statement_signer_handle,
        &config.statement_publisher_handle,
        &config.acknowledgement_authority_handle,
        &config.epoch_witness_store_handle,
    ] {
        if !is_production_runtime_handle(handle) {
            bail!("committed hedging/billing runtime dependency handle is not production-safe");
        }
    }
    for (revision, policy_digest) in [
        (
            config.finalized_query_revision,
            config.finalized_query_policy_digest,
        ),
        (
            config.journal_verifier_revision,
            config.journal_verifier_policy_digest,
        ),
        (
            config.statement_signer_revision,
            config.statement_signer_policy_digest,
        ),
        (
            config.statement_publisher_revision,
            config.statement_publisher_policy_digest,
        ),
        (
            config.acknowledgement_authority_revision,
            config.acknowledgement_authority_policy_digest,
        ),
        (
            config.epoch_witness_store_revision,
            config.epoch_witness_store_policy_digest,
        ),
    ] {
        if revision == 0 || policy_digest == [0; 32] {
            bail!("committed hedging/billing provider qualification is invalid");
        }
    }
    Ok(())
}

fn absolute_leaf(path: &std::path::Path) -> bool {
    path.is_absolute()
        && path.file_name().is_some()
        && !path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
}

fn validate_dependency_handle(label: &str, expected: &str, actual: &str) -> Result<()> {
    if !is_production_runtime_handle(actual) || actual != expected {
        bail!("{label} identity does not match committed hedging/billing configuration");
    }
    Ok(())
}

#[cfg(feature = "telemetry")]
fn record_status_metrics(handle: &HedgingBillingRuntimeHandleV1) {
    let Some(metrics) = iroha_telemetry::metrics::global() else {
        return;
    };
    match handle.status() {
        Ok(status) => metrics.record_sorafs_hedging_billing_runtime_status(
            iroha_telemetry::metrics::SorafsHedgingBillingRuntimeMetricSnapshot {
                runtime: iroha_telemetry::metrics::SorafsRuntimeHealthMetricSnapshot {
                    live: status.live,
                    ready: status.ready,
                    external_dependencies_ready: status.external_dependencies_healthy,
                },
                projection:
                    iroha_telemetry::metrics::SorafsHedgingBillingProjectionMetricSnapshot {
                        automatic_execution_enabled: status.automatic_hedge_execution_enabled,
                        last_tick_fresh: status.last_tick_fresh,
                        finalized_projection_ready: status.finalized_projection_ready,
                    },
                finalized_height: status.service.finalized_height,
                finalized_head_height: status.finalized_head_height,
                finalized_lag_blocks: status.finalized_lag_blocks,
                next_event_sequence: status.service.next_event_sequence,
                ready_for_signing: status.service.ready_for_signing,
                ready_for_publication: status.service.ready_for_publication,
                publication_ambiguous: status.service.publication_ambiguous,
                published: status.service.published,
                acknowledged: status.service.acknowledged,
                dead_letter: status.service.dead_letter,
                hedge_intents: status.service.hedge_intents,
            },
        ),
        Err(_) => metrics.record_sorafs_hedging_billing_runtime_status(
            iroha_telemetry::metrics::SorafsHedgingBillingRuntimeMetricSnapshot {
                runtime: iroha_telemetry::metrics::SorafsRuntimeHealthMetricSnapshot {
                    live: false,
                    ready: false,
                    external_dependencies_ready: false,
                },
                projection:
                    iroha_telemetry::metrics::SorafsHedgingBillingProjectionMetricSnapshot {
                        automatic_execution_enabled: false,
                        last_tick_fresh: false,
                        finalized_projection_ready: false,
                    },
                finalized_height: 0,
                finalized_head_height: 0,
                finalized_lag_blocks: 0,
                next_event_sequence: 0,
                ready_for_signing: 0,
                ready_for_publication: 0,
                publication_ambiguous: 0,
                published: 0,
                acknowledged: 0,
                dead_letter: 1,
                hedge_intents: 0,
            },
        ),
    }
}

#[cfg(not(feature = "telemetry"))]
fn record_status_metrics(_handle: &HedgingBillingRuntimeHandleV1) {}

#[cfg(feature = "telemetry")]
fn record_tick_metric(result: &str) {
    if let Some(metrics) = iroha_telemetry::metrics::global() {
        metrics.inc_sorafs_hedging_billing_runtime_tick(result);
    }
}

#[cfg(not(feature = "telemetry"))]
fn record_tick_metric(_result: &str) {}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
    };

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::account::AccountId;
    use sorafs_manifest::{
        deal::XorQuantity,
        hedging::signed::{
            HEDGING_FEED_BINDING_VERSION_V1, HEDGING_FEED_TRUST_POLICY_VERSION_V1,
            HEDGING_TRUSTED_SIGNER_VERSION_V1, HedgingFeedBindingV1, HedgingTrustedSignerV1,
        },
    };
    use sorafs_node::hedging_billing_service::{
        BILLING_STATEMENT_PUBLISHER_POLICY_VERSION_V1, BILLING_STATEMENT_SIGNER_POLICY_VERSION_V1,
        BillingStatementAcknowledgementAuthorityIdentityV1, BillingStatementAcknowledgementV1,
        BillingStatementAuthoritativePublicationV1, BillingStatementPublicationReceiptV1,
        BillingStatementPublisherIdentityV1, BillingStatementPublisherPolicyV1,
        BillingStatementSignerIdentityV1, BillingStatementSignerPolicyV1,
        HEDGING_BILLING_FINALIZED_PAGE_VERSION_V1, HEDGING_BILLING_JOURNAL_COMMITMENT_VERSION_V1,
        HEDGING_BILLING_POLICY_VERSION_V1, HEDGING_BILLING_TRANSITION_AUTHORITY_VERSION_V1,
        HedgingBillingEpochTransitionV1, HedgingBillingEpochWitnessRecordV1,
        HedgingBillingExternalError, HedgingBillingFinalizedEventPageV1,
        HedgingBillingFinalizedPeriodCloseV1, HedgingBillingJournalCommitmentV1,
        HedgingBillingQueryPositionV1, HedgingBillingRuntimeAdapterIdentityV1,
        HedgingBillingRuntimeProviderReadinessErrorV1, HedgingBillingRuntimeProviderV1,
        HedgingBillingTransitionAuthorityV1, SignedGovernedBillingStatementV1,
    };
    use tempfile::TempDir;

    use super::*;

    const CHAIN_ID: &str = "sorafs-reference-production";
    const QUERY_HANDLE: &str = "ledger.billing.finalized.primary";
    const VERIFIER_HANDLE: &str = "consensus.billing.verifier.primary";
    const SIGNER_HANDLE: &str = "hsm.billing.statement.primary";
    const PUBLISHER_HANDLE: &str = "billing.publisher.primary";
    const ACKNOWLEDGEMENT_HANDLE: &str = "billing.acknowledgement.primary";
    const WITNESS_HANDLE: &str = "sealed.billing.epoch.primary";
    const QUERY_QUALIFICATION: HedgingBillingRuntimeProviderQualificationV1 =
        HedgingBillingRuntimeProviderQualificationV1::new(1, [0xA1; 32]);
    const VERIFIER_QUALIFICATION: HedgingBillingRuntimeProviderQualificationV1 =
        HedgingBillingRuntimeProviderQualificationV1::new(1, [0xA2; 32]);
    const SIGNER_QUALIFICATION: HedgingBillingRuntimeProviderQualificationV1 =
        HedgingBillingRuntimeProviderQualificationV1::new(1, [0xA3; 32]);
    const PUBLISHER_QUALIFICATION: HedgingBillingRuntimeProviderQualificationV1 =
        HedgingBillingRuntimeProviderQualificationV1::new(1, [0xA4; 32]);
    const ACKNOWLEDGEMENT_QUALIFICATION: HedgingBillingRuntimeProviderQualificationV1 =
        HedgingBillingRuntimeProviderQualificationV1::new(1, [0xA5; 32]);
    const WITNESS_QUALIFICATION: HedgingBillingRuntimeProviderQualificationV1 =
        HedgingBillingRuntimeProviderQualificationV1::new(1, [0xA6; 32]);

    #[derive(Debug)]
    struct EmptyFinalizedQuery {
        handle: String,
        ready: Arc<AtomicBool>,
        supplies_period_closes: bool,
        head: Arc<Mutex<HedgingBillingFinalizedCursorV1>>,
    }

    impl HedgingBillingRuntimeProviderV1 for EmptyFinalizedQuery {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            HedgingBillingRuntimeProviderQualificationV1,
            HedgingBillingRuntimeProviderReadinessErrorV1,
        > {
            Ok(QUERY_QUALIFICATION)
        }
    }

    impl HedgingBillingFinalizedQuery for EmptyFinalizedQuery {
        fn identity(
            &self,
        ) -> Result<HedgingBillingRuntimeAdapterIdentityV1, HedgingBillingExternalError> {
            Ok(HedgingBillingRuntimeAdapterIdentityV1 {
                handle: self.handle.clone(),
            })
        }

        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            if self.ready.load(Ordering::Acquire) {
                Ok(())
            } else {
                Err(HedgingBillingExternalError::Unavailable)
            }
        }

        fn supplies_period_closes(&self) -> bool {
            self.supplies_period_closes
        }

        fn finalized_head(
            &self,
        ) -> Result<HedgingBillingFinalizedCursorV1, HedgingBillingExternalError> {
            Ok(*self
                .head
                .lock()
                .map_err(|_| HedgingBillingExternalError::Unavailable)?)
        }

        fn query_finalized_page(
            &self,
            _position: HedgingBillingQueryPositionV1,
            _max_events: u32,
        ) -> Result<Option<HedgingBillingFinalizedEventPageV1>, HedgingBillingExternalError>
        {
            Ok(None)
        }

        fn query_finalized_period_close(
            &self,
            _period_end_unix: u64,
            _position: HedgingBillingQueryPositionV1,
        ) -> Result<Option<HedgingBillingFinalizedPeriodCloseV1>, HedgingBillingExternalError>
        {
            Ok(None)
        }
    }

    #[derive(Debug)]
    struct ReadyJournalVerifier {
        handle: String,
    }

    impl HedgingBillingRuntimeProviderV1 for ReadyJournalVerifier {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            HedgingBillingRuntimeProviderQualificationV1,
            HedgingBillingRuntimeProviderReadinessErrorV1,
        > {
            Ok(VERIFIER_QUALIFICATION)
        }
    }

    impl HedgingBillingJournalVerifier for ReadyJournalVerifier {
        fn identity(
            &self,
        ) -> Result<HedgingBillingRuntimeAdapterIdentityV1, HedgingBillingExternalError> {
            Ok(HedgingBillingRuntimeAdapterIdentityV1 {
                handle: self.handle.clone(),
            })
        }

        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }

        fn verify_page(
            &self,
            _chain_id: &ChainId,
            _previous: Option<HedgingBillingJournalCommitmentV1>,
            _page: &HedgingBillingFinalizedEventPageV1,
        ) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }

        fn verify_period_close(
            &self,
            _chain_id: &ChainId,
            _close: &HedgingBillingFinalizedPeriodCloseV1,
        ) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }

        fn verify_epoch_transition(
            &self,
            _chain_id: &ChainId,
            _transition: &HedgingBillingEpochTransitionV1,
        ) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct PendingStatementSigner {
        identity: BillingStatementSignerIdentityV1,
    }

    impl HedgingBillingRuntimeProviderV1 for PendingStatementSigner {
        fn handle(&self) -> &str {
            &self.identity.provider_handle
        }

        fn qualification(
            &self,
        ) -> Result<
            HedgingBillingRuntimeProviderQualificationV1,
            HedgingBillingRuntimeProviderReadinessErrorV1,
        > {
            Ok(SIGNER_QUALIFICATION)
        }
    }

    impl BillingStatementRuntimeSigner for PendingStatementSigner {
        fn identity(
            &self,
        ) -> Result<BillingStatementSignerIdentityV1, HedgingBillingExternalError> {
            Ok(self.identity.clone())
        }

        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }

        fn sign_digest(&self, _digest: [u8; 32]) -> Result<[u8; 64], HedgingBillingExternalError> {
            Err(HedgingBillingExternalError::Unavailable)
        }
    }

    #[derive(Debug)]
    struct EmptyStatementPublisher {
        identity: BillingStatementPublisherIdentityV1,
    }

    impl HedgingBillingRuntimeProviderV1 for EmptyStatementPublisher {
        fn handle(&self) -> &str {
            &self.identity.provider_handle
        }

        fn qualification(
            &self,
        ) -> Result<
            HedgingBillingRuntimeProviderQualificationV1,
            HedgingBillingRuntimeProviderReadinessErrorV1,
        > {
            Ok(PUBLISHER_QUALIFICATION)
        }
    }

    impl BillingStatementPublisher for EmptyStatementPublisher {
        fn identity(
            &self,
        ) -> Result<BillingStatementPublisherIdentityV1, HedgingBillingExternalError> {
            Ok(self.identity.clone())
        }

        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }

        fn publish(
            &self,
            _idempotency_key: [u8; 32],
            _signed_statement_digest: [u8; 32],
            _statement: &SignedGovernedBillingStatementV1,
        ) -> Result<BillingStatementPublicationReceiptV1, HedgingBillingExternalError> {
            Err(HedgingBillingExternalError::Unavailable)
        }

        fn lookup(
            &self,
            _statement_id: [u8; 32],
        ) -> Result<Option<BillingStatementAuthoritativePublicationV1>, HedgingBillingExternalError>
        {
            Ok(None)
        }
    }

    #[derive(Debug)]
    struct EmptyAcknowledgementAuthority {
        handle: String,
    }

    impl HedgingBillingRuntimeProviderV1 for EmptyAcknowledgementAuthority {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            HedgingBillingRuntimeProviderQualificationV1,
            HedgingBillingRuntimeProviderReadinessErrorV1,
        > {
            Ok(ACKNOWLEDGEMENT_QUALIFICATION)
        }
    }

    impl BillingStatementAcknowledgementAuthority for EmptyAcknowledgementAuthority {
        fn identity(
            &self,
        ) -> Result<BillingStatementAcknowledgementAuthorityIdentityV1, HedgingBillingExternalError>
        {
            Ok(BillingStatementAcknowledgementAuthorityIdentityV1 {
                provider_handle: self.handle.clone(),
            })
        }

        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }

        fn verify(
            &self,
            _statement: &SignedGovernedBillingStatementV1,
            _acknowledgement: &BillingStatementAcknowledgementV1,
        ) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }

        fn record(
            &self,
            _statement: &SignedGovernedBillingStatementV1,
            acknowledgement: &BillingStatementAcknowledgementV1,
        ) -> Result<BillingStatementAcknowledgementV1, HedgingBillingExternalError> {
            Ok(acknowledgement.clone())
        }

        fn lookup(
            &self,
            _statement_id: [u8; 32],
        ) -> Result<Option<BillingStatementAcknowledgementV1>, HedgingBillingExternalError>
        {
            Ok(None)
        }
    }

    #[derive(Debug)]
    struct EmptyEpochWitnessStore {
        handle: String,
    }

    impl HedgingBillingRuntimeProviderV1 for EmptyEpochWitnessStore {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            HedgingBillingRuntimeProviderQualificationV1,
            HedgingBillingRuntimeProviderReadinessErrorV1,
        > {
            Ok(WITNESS_QUALIFICATION)
        }
    }

    impl HedgingBillingEpochWitnessStore for EmptyEpochWitnessStore {
        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }

        fn load_latest(
            &self,
        ) -> Result<Option<HedgingBillingEpochWitnessRecordV1>, HedgingBillingExternalError>
        {
            Ok(None)
        }

        fn load_epoch(
            &self,
            _epoch_sequence: u64,
        ) -> Result<Option<HedgingBillingEpochWitnessRecordV1>, HedgingBillingExternalError>
        {
            Ok(None)
        }

        fn compare_and_swap_latest(
            &self,
            _expected_revision: Option<[u8; 32]>,
            _next: &HedgingBillingEpochWitnessRecordV1,
        ) -> Result<(), HedgingBillingExternalError> {
            Err(HedgingBillingExternalError::Unavailable)
        }
    }

    fn public_key(seed: u8) -> [u8; 32] {
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("deterministic Ed25519 key");
        key.public_key()
            .to_bytes()
            .1
            .try_into()
            .expect("Ed25519 public key is 32 bytes")
    }

    fn account_bytes(seed: u8) -> Vec<u8> {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("deterministic account key")
                .public_key()
                .clone(),
        )
        .canonical_i105()
        .expect("canonical I105 account")
        .into_bytes()
    }

    fn initial_finalized_head() -> HedgingBillingFinalizedCursorV1 {
        HedgingBillingFinalizedCursorV1 {
            height: 1,
            block_hash: [0x44; 32],
            finalized_at_unix: 1_800_000_001,
        }
    }

    fn finality_only_page(
        cursor: HedgingBillingFinalizedCursorV1,
    ) -> HedgingBillingFinalizedEventPageV1 {
        HedgingBillingFinalizedEventPageV1 {
            version: HEDGING_BILLING_FINALIZED_PAGE_VERSION_V1,
            chain_id: ChainId::from(CHAIN_ID),
            start_sequence: 1,
            next_sequence: 1,
            journal_commitment: HedgingBillingJournalCommitmentV1 {
                version: HEDGING_BILLING_JOURNAL_COMMITMENT_VERSION_V1,
                finalized_cursor: cursor,
                journal_next_sequence: 1,
                journal_root: [0xB1; 32],
            },
            append_proof: vec![0xA5],
            inclusion_proof: vec![0xB6],
            events: Vec::new(),
        }
    }

    fn seed_fresh_projection(
        handle: &HedgingBillingRuntimeHandleV1,
        cursor: HedgingBillingFinalizedCursorV1,
    ) {
        handle
            .inner
            .service
            .ingest_finalized_page(&finality_only_page(cursor))
            .expect("seed committed finalized projection");
        handle
            .reconcile_once()
            .expect("record fresh successful projection tick");
        assert!(handle.status().expect("fresh runtime status").ready);
    }

    fn assert_all_runtime_data_methods_unavailable(
        handle: &HedgingBillingRuntimeHandleV1,
        expected_checkpoint_fingerprint: [u8; 32],
    ) {
        let owner_account_id = account_bytes(0x91);
        let statement_id = [0xD1; 32];
        let projection_request = HedgingBillingProjectionPageRequestV1 {
            expected_checkpoint_fingerprint,
            after: None,
            limit: 1,
        };
        assert!(matches!(
            handle.projection_anchor(),
            Err(HedgingBillingRuntimeApiErrorV1::Unavailable)
        ));
        assert!(matches!(
            handle.list_statements(&BillingStatementListRequestV1 {
                owner_account_id: owner_account_id.clone(),
                after_statement_id: None,
                limit: 1,
                expected_checkpoint_fingerprint,
            }),
            Err(HedgingBillingRuntimeApiErrorV1::Unavailable)
        ));
        assert!(matches!(
            handle.published_statement(&BillingPublishedStatementRequestV1 {
                owner_account_id: owner_account_id.clone(),
                statement_id,
                expected_checkpoint_fingerprint,
            }),
            Err(HedgingBillingRuntimeApiErrorV1::Unavailable)
        ));
        assert!(matches!(
            handle.acknowledge_statement(
                &BillingStatementAcknowledgementRequestV1 {
                    expected_checkpoint_fingerprint,
                    statement_id,
                    owner_account_id,
                    request_nonce: [0xD2; 32],
                    authentication_proof: vec![0xAC],
                },
                1_800_000_010,
            ),
            Err(HedgingBillingRuntimeApiErrorV1::Unavailable)
        ));
        assert!(matches!(
            handle.exposure_page(&projection_request),
            Err(HedgingBillingRuntimeApiErrorV1::Unavailable)
        ));
        assert!(matches!(
            handle.hedge_intent_page(&projection_request),
            Err(HedgingBillingRuntimeApiErrorV1::Unavailable)
        ));
        assert!(matches!(
            handle.reconciliation_status(),
            Err(HedgingBillingRuntimeApiErrorV1::Unavailable)
        ));
    }

    fn feed_policy() -> HedgingFeedTrustPolicyV1 {
        HedgingFeedTrustPolicyV1 {
            version: HEDGING_FEED_TRUST_POLICY_VERSION_V1,
            policy_id: [0x51; 32],
            valid_from_unix: 1_800_000_000,
            valid_until_unix: 1_900_000_000,
            max_sample_age_secs: 300,
            max_future_skew_secs: 30,
            signers: vec![HedgingTrustedSignerV1 {
                version: HEDGING_TRUSTED_SIGNER_VERSION_V1,
                signer_id: "pricing-collector-primary".to_owned(),
                public_key: public_key(0x52),
                authorized_feeds: vec![HedgingFeedBindingV1 {
                    version: HEDGING_FEED_BINDING_VERSION_V1,
                    feed_id: "xor-usd-primary".to_owned(),
                    source: "pricing-consortium-primary".to_owned(),
                }],
            }],
            revoked_signer_ids: Vec::new(),
        }
    }

    fn service_policy(feed_policy: &HedgingFeedTrustPolicyV1) -> HedgingBillingServicePolicyV1 {
        HedgingBillingServicePolicyV1 {
            version: HEDGING_BILLING_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            chain_id: ChainId::from(CHAIN_ID),
            billing_policy_digest: [0x61; 32],
            feed_trust_policy_digest: feed_policy.canonical_digest().expect("feed policy digest"),
            billing_epoch_unix: 1_800_000_000,
            billing_period_secs: 3_600,
            payment_due_after_secs: 600,
            max_feed_age_secs: 300,
            max_divergence_bps: 500,
            statement_signer: BillingStatementSignerPolicyV1 {
                version: BILLING_STATEMENT_SIGNER_POLICY_VERSION_V1,
                signer_id: "billing-statement-primary".to_owned(),
                public_key: public_key(0x62),
                valid_from_block_height: 1,
                revoked_at_block_height: None,
            },
            statement_publisher: BillingStatementPublisherPolicyV1 {
                version: BILLING_STATEMENT_PUBLISHER_POLICY_VERSION_V1,
                publisher_id: "billing-publication-primary".to_owned(),
                route_id: "regional-publication-primary".to_owned(),
                public_key: public_key(0x63),
            },
            transition_authority: HedgingBillingTransitionAuthorityV1 {
                version: HEDGING_BILLING_TRANSITION_AUTHORITY_VERSION_V1,
                authority_id: "billing-transition-primary".to_owned(),
                public_key: public_key(0x64),
            },
            epoch_witness_store_handle: WITNESS_HANDLE.to_owned(),
            hedge_intent_threshold_xor: XorQuantity::try_from_micro(5_000_000)
                .expect("hedge threshold"),
            max_hedge_intent_xor: XorQuantity::try_from_micro(100_000_000).expect("hedge ceiling"),
            hedge_intent_ttl_secs: 900,
            hedge_max_slippage_bps: 100,
            max_events_per_page: 64,
            max_retained_source_pages: 1_024,
            max_retained_period_closes: 256,
            max_accounts: 64,
            max_open_accruals: 256,
            max_replay_receipts: 1_024,
            max_statements: 256,
            max_acknowledgements: 256,
            max_hedge_intents: 64,
            max_signing_attempts: 3,
            checkpoint_max_bytes: 16 * 1024 * 1024,
        }
    }

    fn config(
        state_dir: PathBuf,
        policy: &HedgingBillingServicePolicyV1,
    ) -> SorafsHedgingBillingRuntime {
        SorafsHedgingBillingRuntime {
            service_policy_path: state_dir
                .parent()
                .expect("state directory parent")
                .join("hedging-billing-policy.to"),
            service_policy_digest: policy.canonical_digest().expect("service policy digest"),
            state_dir,
            finalized_query_handle: QUERY_HANDLE.to_owned(),
            journal_verifier_handle: VERIFIER_HANDLE.to_owned(),
            statement_signer_handle: SIGNER_HANDLE.to_owned(),
            statement_publisher_handle: PUBLISHER_HANDLE.to_owned(),
            acknowledgement_authority_handle: ACKNOWLEDGEMENT_HANDLE.to_owned(),
            epoch_witness_store_handle: WITNESS_HANDLE.to_owned(),
            finalized_query_revision: QUERY_QUALIFICATION.revision(),
            finalized_query_policy_digest: QUERY_QUALIFICATION.policy_digest(),
            journal_verifier_revision: VERIFIER_QUALIFICATION.revision(),
            journal_verifier_policy_digest: VERIFIER_QUALIFICATION.policy_digest(),
            statement_signer_revision: SIGNER_QUALIFICATION.revision(),
            statement_signer_policy_digest: SIGNER_QUALIFICATION.policy_digest(),
            statement_publisher_revision: PUBLISHER_QUALIFICATION.revision(),
            statement_publisher_policy_digest: PUBLISHER_QUALIFICATION.policy_digest(),
            acknowledgement_authority_revision: ACKNOWLEDGEMENT_QUALIFICATION.revision(),
            acknowledgement_authority_policy_digest: ACKNOWLEDGEMENT_QUALIFICATION.policy_digest(),
            epoch_witness_store_revision: WITNESS_QUALIFICATION.revision(),
            epoch_witness_store_policy_digest: WITNESS_QUALIFICATION.policy_digest(),
            poll_interval: Duration::from_secs(1),
            max_pages_per_tick: 256,
            max_period_closes_per_tick: 32,
            max_delivery_operations_per_tick: 256,
            max_finalized_lag_blocks: 2,
        }
    }

    fn dependencies(
        policy: &HedgingBillingServicePolicyV1,
        query_handle: &str,
        supplies_period_closes: bool,
        query_ready: Arc<AtomicBool>,
    ) -> HedgingBillingRuntimeDependenciesV1 {
        dependencies_with_head(
            policy,
            query_handle,
            supplies_period_closes,
            query_ready,
            Arc::new(Mutex::new(initial_finalized_head())),
        )
    }

    #[test]
    fn production_config_rejects_credential_parameter_and_dummy_handles() {
        let feed_policy = feed_policy();
        let policy = service_policy(&feed_policy);
        for rejected in [
            "https://operator:secret@billing.example",
            "https://billing.example/query?token=secret",
            "https://billing.example/query#fragment",
            "hsm://billing/dummy/signer",
        ] {
            let temp = TempDir::new().expect("tempdir");
            let mut config = config(temp.path().join("state"), &policy);
            config.statement_signer_handle = rejected.to_owned();
            assert!(
                validate_actual_config(&config).is_err(),
                "{rejected:?} must fail before runtime construction"
            );
        }
    }

    fn dependencies_with_head(
        policy: &HedgingBillingServicePolicyV1,
        query_handle: &str,
        supplies_period_closes: bool,
        query_ready: Arc<AtomicBool>,
        head: Arc<Mutex<HedgingBillingFinalizedCursorV1>>,
    ) -> HedgingBillingRuntimeDependenciesV1 {
        HedgingBillingRuntimeDependenciesV1 {
            finalized_query: Arc::new(EmptyFinalizedQuery {
                handle: query_handle.to_owned(),
                ready: query_ready,
                supplies_period_closes,
                head,
            }),
            journal_verifier: Arc::new(ReadyJournalVerifier {
                handle: VERIFIER_HANDLE.to_owned(),
            }),
            statement_signer: Arc::new(PendingStatementSigner {
                identity: BillingStatementSignerIdentityV1 {
                    provider_handle: SIGNER_HANDLE.to_owned(),
                    signer_id: policy.statement_signer.signer_id.clone(),
                    public_key: policy.statement_signer.public_key,
                },
            }),
            statement_publisher: Arc::new(EmptyStatementPublisher {
                identity: BillingStatementPublisherIdentityV1 {
                    provider_handle: PUBLISHER_HANDLE.to_owned(),
                    publisher_id: policy.statement_publisher.publisher_id.clone(),
                    route_id: policy.statement_publisher.route_id.clone(),
                    public_key: policy.statement_publisher.public_key,
                },
            }),
            acknowledgement_authority: Arc::new(EmptyAcknowledgementAuthority {
                handle: ACKNOWLEDGEMENT_HANDLE.to_owned(),
            }),
            epoch_witness_store: Arc::new(EmptyEpochWitnessStore {
                handle: WITNESS_HANDLE.to_owned(),
            }),
        }
    }

    fn fresh_runtime(
        state_dir: PathBuf,
        query_ready: Arc<AtomicBool>,
        head: Arc<Mutex<HedgingBillingFinalizedCursorV1>>,
    ) -> HedgingBillingRuntimeHandleV1 {
        let feed_policy = feed_policy();
        let policy = service_policy(&feed_policy);
        let initial_head = *head.lock().expect("finalized head state");
        let handle = assemble(
            config(state_dir, &policy),
            &ChainId::from(CHAIN_ID),
            policy.clone(),
            &Arc::new(feed_policy),
            dependencies_with_head(&policy, QUERY_HANDLE, true, query_ready, Arc::clone(&head)),
        )
        .expect("assemble committed hedging/billing runtime");
        seed_fresh_projection(&handle, initial_head);
        handle
    }

    #[test]
    fn startup_requires_all_six_runtime_only_adapters() {
        let feed_policy = feed_policy();
        let policy = service_policy(&feed_policy);
        let complete = dependencies(&policy, QUERY_HANDLE, true, Arc::new(AtomicBool::new(true)));

        assert!(
            HedgingBillingRuntimeDependenciesV1::require(
                None,
                Some(Arc::clone(&complete.journal_verifier)),
                Some(Arc::clone(&complete.statement_signer)),
                Some(Arc::clone(&complete.statement_publisher)),
                Some(Arc::clone(&complete.acknowledgement_authority)),
                Some(Arc::clone(&complete.epoch_witness_store)),
            )
            .is_err()
        );
        assert!(
            HedgingBillingRuntimeDependenciesV1::require(
                Some(Arc::clone(&complete.finalized_query)),
                None,
                Some(Arc::clone(&complete.statement_signer)),
                Some(Arc::clone(&complete.statement_publisher)),
                Some(Arc::clone(&complete.acknowledgement_authority)),
                Some(Arc::clone(&complete.epoch_witness_store)),
            )
            .is_err()
        );
        assert!(
            HedgingBillingRuntimeDependenciesV1::require(
                Some(Arc::clone(&complete.finalized_query)),
                Some(Arc::clone(&complete.journal_verifier)),
                None,
                Some(Arc::clone(&complete.statement_publisher)),
                Some(Arc::clone(&complete.acknowledgement_authority)),
                Some(Arc::clone(&complete.epoch_witness_store)),
            )
            .is_err()
        );
        assert!(
            HedgingBillingRuntimeDependenciesV1::require(
                Some(Arc::clone(&complete.finalized_query)),
                Some(Arc::clone(&complete.journal_verifier)),
                Some(Arc::clone(&complete.statement_signer)),
                None,
                Some(Arc::clone(&complete.acknowledgement_authority)),
                Some(Arc::clone(&complete.epoch_witness_store)),
            )
            .is_err()
        );
        assert!(
            HedgingBillingRuntimeDependenciesV1::require(
                Some(Arc::clone(&complete.finalized_query)),
                Some(Arc::clone(&complete.journal_verifier)),
                Some(Arc::clone(&complete.statement_signer)),
                Some(Arc::clone(&complete.statement_publisher)),
                None,
                Some(Arc::clone(&complete.epoch_witness_store)),
            )
            .is_err()
        );
        assert!(
            HedgingBillingRuntimeDependenciesV1::require(
                Some(Arc::clone(&complete.finalized_query)),
                Some(Arc::clone(&complete.journal_verifier)),
                Some(Arc::clone(&complete.statement_signer)),
                Some(Arc::clone(&complete.statement_publisher)),
                Some(Arc::clone(&complete.acknowledgement_authority)),
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn assembly_rejects_adapter_identity_substitution_before_state_open() {
        let temp = TempDir::new().expect("tempdir");
        let state_dir = temp.path().join("must-not-exist");
        let feed_policy = feed_policy();
        let policy = service_policy(&feed_policy);
        let error = assemble(
            config(state_dir.clone(), &policy),
            &ChainId::from(CHAIN_ID),
            policy.clone(),
            &Arc::new(feed_policy),
            dependencies(
                &policy,
                "ledger.billing.finalized.substituted",
                true,
                Arc::new(AtomicBool::new(true)),
            ),
        )
        .expect_err("substituted query identity must fail startup");

        assert!(error.to_string().contains("qualify finalized"));
        assert!(
            !state_dir.exists(),
            "provider qualification must run before private state is opened"
        );
    }

    #[test]
    fn assembly_rejects_all_six_provider_qualification_mismatches_before_state_open() {
        let temp = TempDir::new().expect("tempdir");
        let feed_policy = feed_policy();
        let policy = service_policy(&feed_policy);
        for (provider, expected_context) in [
            ("finalized-query", "qualify finalized"),
            ("journal-verifier", "qualify consensus"),
            ("statement-signer", "qualify billing statement HSM/KMS"),
            ("statement-publisher", "qualify immutable"),
            (
                "acknowledgement-authority",
                "qualify billing acknowledgement",
            ),
            ("epoch-witness-store", "qualify sealed"),
        ] {
            let state_dir = temp.path().join(provider);
            let mut config = config(state_dir.clone(), &policy);
            match provider {
                "finalized-query" => {
                    config.finalized_query_revision =
                        QUERY_QUALIFICATION.revision().saturating_add(1);
                }
                "journal-verifier" => {
                    config.journal_verifier_revision =
                        VERIFIER_QUALIFICATION.revision().saturating_add(1);
                }
                "statement-signer" => {
                    config.statement_signer_revision =
                        SIGNER_QUALIFICATION.revision().saturating_add(1);
                }
                "statement-publisher" => {
                    config.statement_publisher_revision =
                        PUBLISHER_QUALIFICATION.revision().saturating_add(1);
                }
                "acknowledgement-authority" => {
                    config.acknowledgement_authority_revision =
                        ACKNOWLEDGEMENT_QUALIFICATION.revision().saturating_add(1);
                }
                "epoch-witness-store" => {
                    config.epoch_witness_store_revision =
                        WITNESS_QUALIFICATION.revision().saturating_add(1);
                }
                _ => unreachable!("all table entries are explicit"),
            }

            let error = assemble(
                config,
                &ChainId::from(CHAIN_ID),
                policy.clone(),
                &Arc::new(feed_policy.clone()),
                dependencies(&policy, QUERY_HANDLE, true, Arc::new(AtomicBool::new(true))),
            )
            .expect_err("stale provider qualification must fail startup");

            assert!(
                error.to_string().contains(expected_context),
                "{provider} returned an unexpected startup error: {error:#}"
            );
            assert!(
                !state_dir.exists(),
                "{provider} qualification must run before private state is opened"
            );
        }
    }

    #[test]
    fn assembly_rejects_query_without_typed_period_closes_before_state_open() {
        let temp = TempDir::new().expect("tempdir");
        let state_dir = temp.path().join("must-not-exist");
        let feed_policy = feed_policy();
        let policy = service_policy(&feed_policy);
        let error = assemble(
            config(state_dir.clone(), &policy),
            &ChainId::from(CHAIN_ID),
            policy.clone(),
            &Arc::new(feed_policy),
            dependencies(
                &policy,
                QUERY_HANDLE,
                false,
                Arc::new(AtomicBool::new(true)),
            ),
        )
        .expect_err("page-only query adapter must fail startup");

        assert!(error.to_string().contains("period-close"));
        assert!(
            !state_dir.exists(),
            "capability checks must run before private state is opened"
        );
    }

    #[test]
    fn assembly_rejects_unready_adapter_before_state_open() {
        let temp = TempDir::new().expect("tempdir");
        let state_dir = temp.path().join("must-not-exist");
        let feed_policy = feed_policy();
        let policy = service_policy(&feed_policy);
        let error = assemble(
            config(state_dir.clone(), &policy),
            &ChainId::from(CHAIN_ID),
            policy.clone(),
            &Arc::new(feed_policy),
            dependencies(
                &policy,
                QUERY_HANDLE,
                true,
                Arc::new(AtomicBool::new(false)),
            ),
        )
        .expect_err("unready query adapter must fail startup");

        assert!(error.to_string().contains("not ready"));
        assert!(
            !state_dir.exists(),
            "readiness checks must run before private state is opened"
        );
    }

    #[test]
    fn outage_and_restart_are_visible_without_enabling_automatic_execution() {
        let temp = TempDir::new().expect("tempdir");
        let state_dir = temp.path().join("billing-state");
        let feed_policy = feed_policy();
        let policy = service_policy(&feed_policy);
        let config = config(state_dir, &policy);
        let query_ready = Arc::new(AtomicBool::new(true));
        let handle = assemble(
            config.clone(),
            &ChainId::from(CHAIN_ID),
            policy.clone(),
            &Arc::new(feed_policy.clone()),
            dependencies(&policy, QUERY_HANDLE, true, Arc::clone(&query_ready)),
        )
        .expect("assemble committed hedging/billing runtime");

        let initial = handle.status().expect("initial status");
        assert!(!initial.live);
        assert!(!initial.ready);
        assert!(initial.external_dependencies_healthy);
        assert!(!initial.last_tick_healthy);
        assert!(!initial.last_tick_fresh);
        assert!(!initial.finalized_projection_ready);
        assert!(!initial.automatic_hedge_execution_enabled);

        handle.reconcile_once().expect("empty finalized tick");
        let healthy = handle.status().expect("healthy status");
        assert!(healthy.live);
        assert!(
            !healthy.ready,
            "an empty cursor-zero tick must not report production readiness"
        );
        assert!(healthy.external_dependencies_healthy);
        assert!(healthy.last_tick_healthy);
        assert!(healthy.last_tick_fresh);
        assert!(!healthy.finalized_projection_ready);
        assert_eq!(healthy.finalized_head_height, 1);
        assert_eq!(healthy.finalized_lag_blocks, 1);
        assert!(!healthy.automatic_hedge_execution_enabled);

        {
            let mut freshness = handle.last_successful_tick.lock().expect("freshness state");
            *freshness = Instant::now().checked_sub(Duration::from_secs(4));
        }
        let stale = handle.status().expect("stale status");
        assert!(!stale.last_tick_fresh);
        assert!(!stale.ready);
        handle.reconcile_once().expect("freshness recovery tick");
        assert!(handle.status().expect("fresh status").last_tick_fresh);

        query_ready.store(false, Ordering::Release);
        let _error = handle
            .reconcile_once()
            .expect_err("dependency outage must fail the supervised tick");
        let unavailable = handle.status().expect("unavailable status");
        assert!(unavailable.live);
        assert!(!unavailable.ready);
        assert!(!unavailable.external_dependencies_healthy);
        assert!(!unavailable.last_tick_healthy);
        assert!(unavailable.last_tick_fresh);
        assert!(!unavailable.finalized_projection_ready);
        assert!(!unavailable.automatic_hedge_execution_enabled);
        assert_eq!(handle.metrics().successful_ticks, 2);
        assert_eq!(handle.metrics().failed_ticks, 1);

        query_ready.store(true, Ordering::Release);
        handle.reconcile_once().expect("dependency recovery tick");
        assert!(
            !handle.status().expect("recovered status").ready,
            "dependency recovery without finalized progress is still not ready"
        );
        drop(handle);

        let restarted = assemble(
            config,
            &ChainId::from(CHAIN_ID),
            policy.clone(),
            &Arc::new(feed_policy),
            dependencies(&policy, QUERY_HANDLE, true, Arc::new(AtomicBool::new(true))),
        )
        .expect("reopen committed hedging/billing runtime");
        let restart_status = restarted.status().expect("restart status");
        assert!(!restart_status.live);
        assert!(!restart_status.ready);
        assert!(restart_status.external_dependencies_healthy);
        assert!(!restart_status.last_tick_healthy);
        assert!(!restart_status.last_tick_fresh);
        assert!(!restart_status.finalized_projection_ready);
        assert!(!restart_status.automatic_hedge_execution_enabled);
        restarted
            .reconcile_once()
            .expect("restart reconciliation tick");
        assert!(
            !restarted.status().expect("restart status").ready,
            "restart with no finalized projection must remain unready"
        );
    }

    #[test]
    fn data_api_fails_closed_when_finalized_head_exceeds_projection_lag() {
        let temp = TempDir::new().expect("tempdir");
        let query_ready = Arc::new(AtomicBool::new(true));
        let head = Arc::new(Mutex::new(initial_finalized_head()));
        let handle = fresh_runtime(
            temp.path().join("billing-state"),
            Arc::clone(&query_ready),
            Arc::clone(&head),
        );
        let anchor = handle.projection_anchor().expect("fresh projection anchor");

        set_finalized_head(&head, 4, [0x47; 32], 1_800_000_004);
        assert!(
            handle.status().expect("cached runtime status").ready,
            "the request guard must not rely on the worker's cached head"
        );
        assert_all_runtime_data_methods_unavailable(&handle, anchor.checkpoint_fingerprint);
    }

    #[test]
    fn acknowledgement_fence_rejects_any_concurrent_finalized_head_change() {
        let temp = TempDir::new().expect("tempdir");
        let query_ready = Arc::new(AtomicBool::new(true));
        let head = Arc::new(Mutex::new(initial_finalized_head()));
        let handle = fresh_runtime(
            temp.path().join("billing-state"),
            query_ready,
            Arc::clone(&head),
        );

        let error = handle
            .with_fresh_projection(|_service, pre_commit_fence| {
                set_finalized_head(&head, 2, [0x45; 32], 1_800_000_002);
                pre_commit_fence().map_err(|_| HedgingBillingRuntimeApiErrorV1::Unavailable)
            })
            .expect_err("a head change inside the permitted lag still invalidates the ACK fence");
        assert_eq!(error, HedgingBillingRuntimeApiErrorV1::Unavailable);
    }

    #[test]
    fn data_api_fails_closed_on_live_finalized_query_failure() {
        let temp = TempDir::new().expect("tempdir");
        let query_ready = Arc::new(AtomicBool::new(true));
        let head = Arc::new(Mutex::new(initial_finalized_head()));
        let handle = fresh_runtime(
            temp.path().join("billing-state"),
            Arc::clone(&query_ready),
            head,
        );
        let anchor = handle.projection_anchor().expect("fresh projection anchor");

        query_ready.store(false, Ordering::Release);
        assert!(
            handle.status().expect("cached runtime status").ready,
            "the request guard must actively probe the qualified query provider"
        );
        assert_all_runtime_data_methods_unavailable(&handle, anchor.checkpoint_fingerprint);
        assert!(!handle.status().expect("failed live probe status").ready);
    }

    #[test]
    fn data_api_fails_closed_after_tick_freshness_expires() {
        let temp = TempDir::new().expect("tempdir");
        let query_ready = Arc::new(AtomicBool::new(true));
        let head = Arc::new(Mutex::new(initial_finalized_head()));
        let handle = fresh_runtime(temp.path().join("billing-state"), query_ready, head);
        let anchor = handle.projection_anchor().expect("fresh projection anchor");

        {
            let mut freshness = handle.last_successful_tick.lock().expect("freshness state");
            *freshness = Instant::now().checked_sub(Duration::from_secs(4));
        }
        assert!(!handle.status().expect("expired runtime status").ready);
        assert_all_runtime_data_methods_unavailable(&handle, anchor.checkpoint_fingerprint);
    }

    fn set_finalized_head(
        head: &Mutex<HedgingBillingFinalizedCursorV1>,
        height: u64,
        block_hash: [u8; 32],
        finalized_at_unix: u64,
    ) {
        *head.lock().expect("head state") = HedgingBillingFinalizedCursorV1 {
            height,
            block_hash,
            finalized_at_unix,
        };
    }

    #[test]
    fn finalized_head_regression_and_equivocation_fail_closed() {
        let temp = TempDir::new().expect("tempdir");
        let feed_policy = feed_policy();
        let policy = service_policy(&feed_policy);
        let head = Arc::new(Mutex::new(HedgingBillingFinalizedCursorV1 {
            height: 2,
            block_hash: [0x42; 32],
            finalized_at_unix: 1_800_000_002,
        }));
        let handle = assemble(
            config(temp.path().join("billing-state"), &policy),
            &ChainId::from(CHAIN_ID),
            policy.clone(),
            &Arc::new(feed_policy),
            dependencies_with_head(
                &policy,
                QUERY_HANDLE,
                true,
                Arc::new(AtomicBool::new(true)),
                Arc::clone(&head),
            ),
        )
        .expect("assemble committed hedging/billing runtime");

        handle
            .reconcile_once()
            .expect("observe initial finalized head");
        assert_eq!(
            handle
                .status()
                .expect("initial status")
                .finalized_head_height,
            2
        );
        assert_eq!(handle.metrics().successful_ticks, 1);
        assert_eq!(handle.metrics().failed_ticks, 0);
        let anchor_before_failure = handle
            .inner
            .service
            .api_projection_anchor()
            .expect("projection anchor before invalid head");

        set_finalized_head(&head, 2, [0x43; 32], 1_800_000_002);
        let equivocation = handle
            .reconcile_once()
            .expect_err("same-height finalized-head equivocation must fail");
        assert!(
            equivocation
                .to_string()
                .contains("regressed or equivocated")
        );
        let failed = handle.status().expect("failed status");
        assert!(!failed.last_tick_healthy);
        assert!(!failed.external_dependencies_healthy);
        assert_eq!(failed.finalized_head_height, 2);
        assert_eq!(handle.metrics().successful_ticks, 1);
        assert_eq!(handle.metrics().failed_ticks, 1);
        assert_eq!(
            handle
                .inner
                .service
                .api_projection_anchor()
                .expect("projection anchor after invalid head"),
            anchor_before_failure,
            "invalid head observations must be rejected before projection work"
        );

        set_finalized_head(&head, 3, [0x44; 32], 1_800_000_003);
        handle
            .reconcile_once()
            .expect("a later authenticated head recovers");
        assert_eq!(
            handle
                .status()
                .expect("recovered status")
                .finalized_head_height,
            3
        );
        assert_eq!(handle.metrics().successful_ticks, 2);
        assert_eq!(handle.metrics().failed_ticks, 1);

        set_finalized_head(&head, 2, [0x42; 32], 1_800_000_004);
        let regression = handle
            .reconcile_once()
            .expect_err("finalized-head height regression must fail");
        assert!(regression.to_string().contains("regressed or equivocated"));
        assert_eq!(
            handle
                .status()
                .expect("regressed status")
                .finalized_head_height,
            3
        );
        assert_eq!(handle.metrics().successful_ticks, 2);
        assert_eq!(handle.metrics().failed_ticks, 2);
        assert_eq!(
            handle
                .inner
                .service
                .api_projection_anchor()
                .expect("projection anchor after regression"),
            anchor_before_failure
        );
    }

    #[test]
    fn runtime_api_is_object_safe_and_exposes_only_bounded_node_owned_projections() {
        let temp = TempDir::new().expect("tempdir");
        let handle = fresh_runtime(
            temp.path().join("billing-state"),
            Arc::new(AtomicBool::new(true)),
            Arc::new(Mutex::new(initial_finalized_head())),
        );
        let api: &dyn HedgingBillingRuntimeApiV1 = &handle;
        let anchor = api
            .projection_anchor()
            .expect("node-owned projection anchor");
        assert_eq!(
            anchor.retention_scope,
            sorafs_node::hedging_billing_service::HedgingBillingRetentionScopeV1::ActiveEpochOnly
        );
        let status = api.daemon_status().expect("payload-free daemon status");
        assert!(!status.automatic_hedge_execution_enabled);
        let reconciliation = api
            .reconciliation_status()
            .expect("payload-free reconciliation status");
        assert_eq!(reconciliation.anchor, anchor);
        assert_eq!(reconciliation.pending_delivery_operations, 0);
        assert_eq!(api.daemon_metrics().successful_ticks, 1);
    }
}

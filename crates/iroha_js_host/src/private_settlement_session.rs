//! Thin opaque N-API custody over the sole native confidential-payment session owner.

use iroha::{
    client::{PrivateSettlementRunOptions, VerifiedPrivateSettlementOutcome, run_native_worker},
    workflow::WorkflowRunOptions,
};
use iroha_private_settlement::{
    NATIVE_PRIVATE_SETTLEMENT_DESCRIPTOR_MAX_BYTES, NATIVE_PRIVATE_SETTLEMENT_TERMINAL_ALLOWANCE,
    NativePrivateSettlementCommand, NativePrivateSettlementCommandError,
    NativePrivateSettlementHost, NativePrivateSettlementLimits, NativePrivateSettlementOutcome,
    NativePrivateSettlementSessionError, NativePrivateSettlementStatus,
    NativePrivateSettlementTicket,
};
use napi::bindgen_prelude::{Buffer, PromiseRaw};
use napi::{Env, JsValue, Unknown};
use napi_derive::napi;
use std::{
    num::{NonZeroU16, NonZeroUsize},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Semaphore;

/// Complete source descriptor limit, checked natively before its first retained copy.
#[napi(js_name = "privateSettlementDescriptorMaxBytes")]
#[expect(
    clippy::cast_possible_truncation,
    reason = "the assertion below keeps the native descriptor limit within u32"
)]
pub const DESCRIPTOR_MAX_BYTES: u32 = NATIVE_PRIVATE_SETTLEMENT_DESCRIPTOR_MAX_BYTES as u32;
const _: () = assert!(NATIVE_PRIVATE_SETTLEMENT_DESCRIPTOR_MAX_BYTES <= u32::MAX as usize);

/// Canonical native local observation defaults. These never change committed validity.
#[napi(object)]
#[derive(Clone, Copy)]
pub struct JsPrivateSettlementRunOptions {
    /// Maximum native proof reads, including authenticated prefix advancement.
    pub max_finality_proof_reads: u32,
    /// Local observation timeout in milliseconds.
    pub observation_timeout_ms: u32,
    /// Nonzero interval between committed-outcome reads.
    pub poll_interval_ms: u32,
    /// Explicit permission for one identical persisted-byte retransmission.
    pub retransmit_identical_bytes: bool,
    /// Explicit permission for a replacement after authenticated rejection.
    pub retry_rejected_step: bool,
}
/// Read the sole native defaults owner without constructing a payment or session.
#[napi(js_name = "privateSettlementDefaultRunOptions")]
pub fn default_run_options() -> napi::Result<JsPrivateSettlementRunOptions> {
    let original = PrivateSettlementRunOptions::default();
    Ok(JsPrivateSettlementRunOptions {
        max_finality_proof_reads: u32::from(original.max_finality_proof_reads),
        observation_timeout_ms: u32::try_from(original.workflow.observation_timeout.as_millis())
            .map_err(|_| invalid())?,
        poll_interval_ms: u32::try_from(original.workflow.poll_interval.as_millis())
            .map_err(|_| invalid())?,
        retransmit_identical_bytes: original.workflow.retransmit_identical_bytes,
        retry_rejected_step: original.workflow.retry_rejected_step,
    })
}

fn invalid() -> napi::Error {
    napi::Error::new(
        napi::Status::InvalidArg,
        "native private settlement input invalid",
    )
}
// N-API integer conversions truncate and wrap JavaScript numbers. Keep the original
// double until every finite/integral/range check has completed, then convert once.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is a finite integer within [minimum, maximum] before the single conversion"
)]
fn exact_u32(value: f64, minimum: u32, maximum: u32) -> napi::Result<u32> {
    if !value.is_finite()
        || value.fract() != 0.0
        || value < f64::from(minimum)
        || value > f64::from(maximum)
    {
        return Err(invalid());
    }
    Ok(value as u32)
}
fn local(error: NativePrivateSettlementSessionError) -> napi::Error {
    // Display is a fixed closed category and contains no provider diagnostics or source paths.
    napi::Error::new(napi::Status::GenericFailure, error.to_string())
}
fn unavailable() -> napi::Error {
    local(NativePrivateSettlementSessionError::Unavailable)
}
// No Rust slice or typed-array conversion exists until native metadata rejects shared or
// detached backing. The bounded native copy runs synchronously without JavaScript re-entry.
// An ordinary resizable ArrayBuffer cannot change extent during this callback; no source
// view or backing pointer survives into asynchronous work.
#[allow(unsafe_code)]
fn with_descriptor<R>(
    env: &Env,
    value: Unknown<'_>,
    copy: impl FnOnce(&[u8]) -> napi::Result<R>,
) -> napi::Result<R> {
    let mut kind = 0;
    let mut length = 0;
    let mut backing = std::ptr::null_mut();
    let mut offset = 0;
    let mut ordinary = false;
    // SAFETY: these handles belong to this callback. N-API metadata reads do not invoke JS;
    // no pointer is requested, dereferenced or borrowed before ordinary-backing admission.
    unsafe {
        if napi::sys::napi_get_typedarray_info(
            env.raw(),
            value.raw(),
            &raw mut kind,
            &raw mut length,
            std::ptr::null_mut(),
            &raw mut backing,
            &raw mut offset,
        ) != napi::sys::Status::napi_ok
            || kind != napi::sys::TypedarrayType::uint8_array
            || length == 0
            || length > NATIVE_PRIVATE_SETTLEMENT_DESCRIPTOR_MAX_BYTES
            || napi::sys::napi_is_arraybuffer(env.raw(), backing, &raw mut ordinary)
                != napi::sys::Status::napi_ok
            || !ordinary
        {
            return Err(invalid());
        }
        let mut pointer = std::ptr::null_mut();
        let mut backing_length = 0;
        if napi::sys::napi_get_arraybuffer_info(
            env.raw(),
            backing,
            &raw mut pointer,
            &raw mut backing_length,
        ) != napi::sys::Status::napi_ok
            || pointer.is_null()
            || offset
                .checked_add(length)
                .is_none_or(|end| end > backing_length)
        {
            return Err(invalid());
        }
        copy(std::slice::from_raw_parts(
            pointer.cast::<u8>().add(offset),
            length,
        ))
    }
}

fn status(value: NativePrivateSettlementStatus) -> &'static str {
    match value {
        NativePrivateSettlementStatus::Reserved => "reserved",
        NativePrivateSettlementStatus::Pending => "pending",
        NativePrivateSettlementStatus::Ready => "ready",
        NativePrivateSettlementStatus::Idle => "idle",
        NativePrivateSettlementStatus::Closing => "closing",
        NativePrivateSettlementStatus::Closed => "closed",
        NativePrivateSettlementStatus::Unavailable => "unavailable",
    }
}

/// Native-issued host. There is no JavaScript constructor or imported handle decoder.
#[napi(js_name = "PrivateSettlementNativeHost")]
pub struct JsPrivateSettlementHost {
    host: NativePrivateSettlementHost,
    maximum_result_bytes: usize,
}

/// Create a bounded owner on the N-API runtime already supplied by the installed host.
/// The call runs inside that runtime's context and only spawns the owner's supervisor.
/// Keep that runtime alive and await shutdown before destroying the JavaScript environment.
#[napi(js_name = "privateSettlementCreateHost", async_runtime)]
pub fn create_host(
    maximum_sessions: f64,
    maximum_original_bytes: f64,
    maximum_artifact_bytes: f64,
    maximum_prepared_bytes: f64,
) -> napi::Result<JsPrivateSettlementHost> {
    let maximum_sessions = exact_u32(maximum_sessions, 1, u32::from(u16::MAX))?;
    let maximum_original_bytes = exact_u32(maximum_original_bytes, 1, u32::MAX)?;
    let maximum_artifact_bytes = exact_u32(maximum_artifact_bytes, 1, u32::MAX)?;
    let maximum_prepared_bytes = exact_u32(maximum_prepared_bytes, 1, u32::MAX)?;
    let limits = NativePrivateSettlementLimits {
        maximum_sessions: NonZeroU16::new(u16::try_from(maximum_sessions).map_err(|_| invalid())?)
            .ok_or_else(invalid)?,
        maximum_original_bytes: NonZeroUsize::new(maximum_original_bytes as usize)
            .ok_or_else(invalid)?,
        maximum_artifact_bytes: NonZeroUsize::new(maximum_artifact_bytes as usize)
            .ok_or_else(invalid)?,
        maximum_prepared_bytes: NonZeroUsize::new(maximum_prepared_bytes as usize)
            .ok_or_else(invalid)?,
    };
    let runtime = tokio::runtime::Handle::try_current().map_err(|_| unavailable())?;
    Ok(JsPrivateSettlementHost {
        host: NativePrivateSettlementHost::new(runtime, limits).map_err(local)?,
        maximum_result_bytes: limits
            .maximum_prepared_bytes
            .get()
            .max(NATIVE_PRIVATE_SETTLEMENT_TERMINAL_ALLOWANCE),
    })
}
#[napi]
impl JsPrivateSettlementHost {
    /// Admit before copying or decoding the complete borrowed native file descriptor.
    #[napi(js_name = "openFiles")]
    pub fn open_files(
        &self,
        env: Env,
        descriptor: Unknown<'_>,
    ) -> napi::Result<JsPrivateSettlementTicket> {
        let admission = self.host.admit().map_err(local)?;
        let ticket = with_descriptor(&env, descriptor, |bytes| {
            admission.open_files(bytes).map_err(local)
        })?;
        Ok(JsPrivateSettlementTicket {
            host: self.host.clone(),
            ticket,
            maximum_result_bytes: self.maximum_result_bytes,
        })
    }
    /// Drain every accepted command and original native cleanup before returning.
    #[napi]
    pub fn shutdown<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
        let drain = self.host.shutdown();
        env.spawn_future(async move { drain.await.map_err(local) })
    }
}

/// Native-issued exact accepted command, including its original session owner.
#[napi(js_name = "PrivateSettlementNativeTicket")]
pub struct JsPrivateSettlementTicket {
    host: NativePrivateSettlementHost,
    ticket: NativePrivateSettlementTicket,
    maximum_result_bytes: usize,
}
#[napi]
impl JsPrivateSettlementTicket {
    /// Explicitly accept one existing workflow operation; no implicit retransmission or retry.
    #[napi]
    pub fn start(
        &self,
        command: f64,
        max_finality_proof_reads: f64,
        observation_timeout_ms: f64,
        poll_interval_ms: f64,
        retransmit_identical_bytes: bool,
        retry_rejected_step: bool,
    ) -> napi::Result<Self> {
        let command = match command {
            0.0 => NativePrivateSettlementCommand::Prepare,
            1.0 => NativePrivateSettlementCommand::Prepared,
            2.0 => NativePrivateSettlementCommand::Run,
            3.0 => NativePrivateSettlementCommand::Recover,
            _ => return Err(invalid()),
        };
        let max_finality_proof_reads = exact_u32(max_finality_proof_reads, 0, u32::from(u16::MAX))?;
        let observation_timeout_ms = exact_u32(observation_timeout_ms, 0, u32::MAX)?;
        let poll_interval_ms = exact_u32(poll_interval_ms, 1, u32::MAX)?;
        let options = PrivateSettlementRunOptions {
            workflow: WorkflowRunOptions {
                observation_timeout: Duration::from_millis(observation_timeout_ms.into()),
                poll_interval: Duration::from_millis(poll_interval_ms.into()),
                retransmit_identical_bytes,
                retry_rejected_step,
            },
            max_finality_proof_reads: u16::try_from(max_finality_proof_reads)
                .map_err(|_| invalid())?,
        };
        let ticket = self
            .host
            .start(self.ticket.session(), command, options)
            .map_err(local)?;
        Ok(Self {
            host: self.host.clone(),
            ticket,
            maximum_result_bytes: self.maximum_result_bytes,
        })
    }
    /// Stage exact native reservation bytes and governed openings; never called by Recover.
    #[napi(js_name = "stageFiles")]
    pub fn stage_files(&self, env: Env, descriptor: Unknown<'_>) -> napi::Result<Self> {
        let ticket = with_descriptor(&env, descriptor, |bytes| {
            self.host
                .stage_files(self.ticket.session(), bytes)
                .map_err(local)
        })?;
        Ok(Self {
            host: self.host.clone(),
            ticket,
            maximum_result_bytes: self.maximum_result_bytes,
        })
    }
    /// Read lifecycle only. Ready does not imply a successful payment.
    #[napi]
    pub fn status(&self) -> napi::Result<String> {
        self.host
            .status(self.ticket)
            .map(|value| status(value).into())
            .map_err(local)
    }
    /// Waiter cancellation leaves the accepted native command owned by the same supervisor.
    #[napi]
    pub async fn wait(&self) -> napi::Result<String> {
        self.host
            .wait(self.ticket)
            .await
            .map(|value| status(value).into())
            .map_err(local)
    }
    /// Transfer the original result once into a native-issued opaque owner, never a decoded DTO.
    #[napi]
    pub fn take(&self) -> napi::Result<JsPrivateSettlementOutcome> {
        let outcome = self.host.take(self.ticket).map_err(local)?;
        Ok(JsPrivateSettlementOutcome {
            host: self.host.clone(),
            outcome: Arc::new(Mutex::new(outcome)),
            native: Arc::new(Semaphore::new(1)),
            maximum_result_bytes: self.maximum_result_bytes,
        })
    }
    /// Close this original session after all accepted work and native cleanup drain.
    #[napi]
    pub fn close<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
        let drain = self.host.close(self.ticket.session());
        env.spawn_future(async move { drain.await.map_err(local) })
    }
}

/// Redacted progress data with no proof, retry permission or receipt authority.
#[napi(object, use_nullable = true)]
pub struct JsPrivateSettlementProgress {
    /// Exact original workflow identifier.
    pub workflow_id: String,
    /// Closed native workflow state.
    pub status: String,
    /// Exact decimal journal sequence; never a lossy JavaScript number.
    pub sequence: String,
    /// Local cancellation request; it never releases money or notes.
    pub cancel_requested: bool,
    /// First unfinished original step, preserving absence.
    pub current_step: Option<u32>,
    /// Closed original failure code without diagnostic text.
    pub failure_code: Option<String>,
    /// Completed original step indices.
    pub confirmed_steps: Vec<u32>,
    /// Possibly effective unresolved original step indices.
    pub unresolved_steps: Vec<u32>,
    /// Failed original step indices.
    pub failed_steps: Vec<u32>,
}
fn progress(outcome: &NativePrivateSettlementOutcome) -> Option<JsPrivateSettlementProgress> {
    let value = match outcome {
        NativePrivateSettlementOutcome::Pending(value) => value.as_ref(),
        NativePrivateSettlementOutcome::Verified { progress, .. } => progress.as_ref(),
        NativePrivateSettlementOutcome::ReconciliationPending(value) => value.progress(),
        _ => None,
    }?;
    Some(JsPrivateSettlementProgress {
        workflow_id: value.workflow_id.to_hex(),
        status: value.status.id().into(),
        sequence: value.sequence.to_string(),
        cancel_requested: value.cancel_requested,
        current_step: value.current_step,
        failure_code: value.failure_code.map(|code| code.id().into()),
        confirmed_steps: value.partial_completion.confirmed_steps.clone(),
        unresolved_steps: value.partial_completion.unresolved_steps.clone(),
        failed_steps: value.partial_completion.failed_steps.clone(),
    })
}

/// Native-issued result. Public data bytes have no decoder back into verified authority.
#[napi(js_name = "PrivateSettlementNativeOutcome")]
pub struct JsPrivateSettlementOutcome {
    host: NativePrivateSettlementHost,
    outcome: Arc<Mutex<NativePrivateSettlementOutcome>>,
    native: Arc<Semaphore>,
    maximum_result_bytes: usize,
}
#[napi]
impl JsPrivateSettlementOutcome {
    /// Closed discriminant selected from the original typed result.
    #[napi]
    pub fn kind(&self) -> napi::Result<String> {
        let outcome = self
            .outcome
            .try_lock()
            .map_err(|_| local(NativePrivateSettlementSessionError::Busy))?;
        Ok(kind(&outcome).into())
    }
    /// Closed operational category with no private path, key, endpoint or provider text.
    #[napi(js_name = "failureCode")]
    pub fn failure_code(&self) -> napi::Result<Option<String>> {
        let outcome = self
            .outcome
            .try_lock()
            .map_err(|_| local(NativePrivateSettlementSessionError::Busy))?;
        Ok(match &*outcome {
            NativePrivateSettlementOutcome::Failed(
                NativePrivateSettlementCommandError::Operation(error),
            ) => Some(error.workflow_code().id().into()),
            NativePrivateSettlementOutcome::Failed(
                NativePrivateSettlementCommandError::Panicked,
            ) => Some("NativeCommandPanicked".into()),
            NativePrivateSettlementOutcome::Failed(
                NativePrivateSettlementCommandError::Unavailable,
            ) => Some("NativeCommandUnavailable".into()),
            NativePrivateSettlementOutcome::ReconciliationPending(pending) => {
                Some(pending.cause().workflow_code().id().into())
            }
            _ => None,
        })
    }
    /// Borrow bounded redacted progress, preserving an absent original journal as null.
    #[napi]
    pub fn progress(&self) -> napi::Result<Option<JsPrivateSettlementProgress>> {
        let outcome = self
            .outcome
            .try_lock()
            .map_err(|_| local(NativePrivateSettlementSessionError::Busy))?;
        Ok(progress(&outcome))
    }
    /// Canonical Norito DATA only: prepared material vector or original public terminal receipt.
    /// Other outcomes return an empty byte sequence; no data decoder creates native authority.
    /// The opaque original survives serialization/allocation failure for another bounded attempt.
    #[napi]
    pub fn data<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, Buffer>> {
        let lease = self.host.admit_export().map_err(local)?;
        let permit = self
            .native
            .clone()
            .try_acquire_owned()
            .map_err(|_| local(NativePrivateSettlementSessionError::Busy))?;
        let original = self.outcome.clone();
        let maximum = self.maximum_result_bytes;
        env.spawn_future(async move {
            let (result, _) = run_native_worker(permit, (original.clone(), lease), move || {
                let outcome = original.lock().map_err(|_| ())?;
                data(&outcome, maximum)
            })
            .await
            .map_err(|_| unavailable())?;
            result
                .map(Buffer::from)
                .map_err(|()| local(NativePrivateSettlementSessionError::Capacity))
        })
    }
}
fn terminal_kind(outcome: &VerifiedPrivateSettlementOutcome, reconciliation: bool) -> &'static str {
    match (outcome, reconciliation) {
        (VerifiedPrivateSettlementOutcome::Settled(_), false) => "settled",
        (VerifiedPrivateSettlementOutcome::Aborted(_), false) => "aborted",
        (VerifiedPrivateSettlementOutcome::Settled(_), true) => "reconciliation_pending_settled",
        (VerifiedPrivateSettlementOutcome::Aborted(_), true) => "reconciliation_pending_aborted",
    }
}
fn kind(outcome: &NativePrivateSettlementOutcome) -> &'static str {
    match outcome {
        NativePrivateSettlementOutcome::Opened => "opened",
        NativePrivateSettlementOutcome::Staged => "staged",
        NativePrivateSettlementOutcome::Prepared(_) => "prepared",
        NativePrivateSettlementOutcome::NotPrepared => "not_prepared",
        NativePrivateSettlementOutcome::Pending(_) => "pending",
        NativePrivateSettlementOutcome::Verified { outcome, .. } => terminal_kind(outcome, false),
        NativePrivateSettlementOutcome::ReconciliationPending(pending) => {
            terminal_kind(pending.outcome(), true)
        }
        NativePrivateSettlementOutcome::Failed(_) => "failed",
    }
}
fn encoded<T: norito::NoritoSerialize>(value: &T, maximum: usize) -> Result<Vec<u8>, ()> {
    if norito::canonical_frame_len(value).map_err(|_| ())? > maximum {
        return Err(());
    }
    norito::encode_canonical(value).map_err(|_| ())
}
fn terminal_data(
    outcome: &VerifiedPrivateSettlementOutcome,
    maximum: usize,
) -> Result<Vec<u8>, ()> {
    match outcome {
        VerifiedPrivateSettlementOutcome::Settled(receipt) => encoded(receipt.receipt(), maximum),
        VerifiedPrivateSettlementOutcome::Aborted(receipt) => encoded(receipt.receipt(), maximum),
    }
}
fn data(outcome: &NativePrivateSettlementOutcome, maximum: usize) -> Result<Vec<u8>, ()> {
    match outcome {
        NativePrivateSettlementOutcome::Prepared(prepared) => {
            prepared.encode_materials(maximum).map_err(|_| ())
        }
        NativePrivateSettlementOutcome::Verified { outcome, .. } => terminal_data(outcome, maximum),
        NativePrivateSettlementOutcome::ReconciliationPending(pending) => {
            terminal_data(pending.outcome(), maximum)
        }
        _ => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_controls_are_finite_exact_and_bounded_before_integer_conversion() {
        for (minimum, maximum) in [
            (0, u32::from(u16::MAX)),
            (1, u32::from(u16::MAX)),
            (0, u32::MAX),
            (1, u32::MAX),
        ] {
            for exact in [minimum, 1, maximum] {
                assert_eq!(
                    exact_u32(f64::from(exact), minimum, maximum).unwrap(),
                    exact
                );
            }
            for invalid in [
                f64::from(minimum) - 1.0,
                f64::from(maximum) + 1.0,
                1.5,
                f64::from(maximum) - 0.5,
                f64::from(u32::MAX) + 2.0,
                f64::NAN,
                f64::INFINITY,
                f64::NEG_INFINITY,
            ] {
                let error = exact_u32(invalid, minimum, maximum).unwrap_err();
                assert_eq!(error.status, napi::Status::InvalidArg);
                assert_eq!(error.reason, "native private settlement input invalid");
            }
        }
        assert_eq!(exact_u32(-0.0, 0, u32::MAX).unwrap(), 0);
        assert!(exact_u32(-0.0, 1, u32::MAX).is_err());
    }
    #[test]
    fn create_host_checks_limits_before_binding_the_current_runtime() {
        let descriptor = f64::from(DESCRIPTOR_MAX_BYTES);
        assert_eq!(
            usize::try_from(DESCRIPTOR_MAX_BYTES).unwrap(),
            NATIVE_PRIVATE_SETTLEMENT_DESCRIPTOR_MAX_BYTES
        );
        for limits in [
            (0.0, descriptor, 1024.0, 1024.0),
            (f64::from(u16::MAX) + 1.0, descriptor, 1024.0, 1024.0),
            (1.0, 0.0, 1024.0, 1024.0),
            (1.0, descriptor, 1.5, 1024.0),
            (1.0, descriptor, 1024.0, f64::NAN),
        ] {
            let Err(error) = create_host(limits.0, limits.1, limits.2, limits.3) else {
                panic!("invalid limits must be refused before a runtime is bound");
            };
            assert_eq!(error.status, napi::Status::InvalidArg);
        }
        let Err(error) = create_host(1.0, descriptor, 1024.0, 1024.0) else {
            panic!("valid limits still need the installed runtime");
        };
        assert_eq!(error.status, napi::Status::GenericFailure);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let host = runtime
            .block_on(async { create_host(1.0, descriptor, 1024.0, 1024.0) })
            .unwrap();
        assert_eq!(
            host.maximum_result_bytes,
            NATIVE_PRIVATE_SETTLEMENT_TERMINAL_ALLOWANCE.max(1024)
        );
        runtime.block_on(host.host.shutdown()).unwrap();
    }
    #[test]
    fn default_options_and_redacted_progress_keep_the_native_owner_and_exact_sequence() {
        use iroha::workflow::{
            PartialCompletionV1, WorkflowId, WorkflowKindV1, WorkflowProgressV1, WorkflowStatusV1,
        };
        let defaults = default_run_options().unwrap();
        let original = PrivateSettlementRunOptions::default();
        assert_eq!(
            defaults.max_finality_proof_reads,
            u32::from(original.max_finality_proof_reads)
        );
        assert_eq!(
            u128::from(defaults.observation_timeout_ms),
            original.workflow.observation_timeout.as_millis()
        );
        assert_eq!(
            u128::from(defaults.poll_interval_ms),
            original.workflow.poll_interval.as_millis()
        );
        assert_eq!(
            defaults.retransmit_identical_bytes,
            original.workflow.retransmit_identical_bytes
        );
        assert_eq!(
            defaults.retry_rejected_step,
            original.workflow.retry_rejected_step
        );
        let original = WorkflowProgressV1 {
            workflow_id: WorkflowId::from_bytes([7; 32]),
            kind: WorkflowKindV1::PrivateSettlement,
            status: WorkflowStatusV1::InProgress,
            sequence: u64::MAX,
            cancel_requested: false,
            atomicity: "per_transaction_only",
            current_step: Some(2),
            failure_code: None,
            steps: Vec::new(),
            partial_completion: PartialCompletionV1 {
                confirmed_steps: vec![0, 1],
                unresolved_steps: vec![2],
                ..Default::default()
            },
        };
        let owner = NativePrivateSettlementOutcome::Pending(Some(original));
        let projected = progress(&owner).unwrap();
        assert_eq!(projected.sequence, u64::MAX.to_string());
        assert_eq!(projected.status, "in_progress");
        assert_eq!(projected.current_step, Some(2));
        assert_eq!(projected.confirmed_steps, [0, 1]);
        assert_eq!(projected.unresolved_steps, [2]);
        assert_eq!(projected.failure_code, None);
        assert!(data(&owner, 0).unwrap().is_empty());
    }
    #[test]
    fn redacted_kind_and_data_refusal_preserve_original_outcome() {
        let pending = NativePrivateSettlementOutcome::Pending(None);
        assert_eq!(kind(&pending), "pending");
        assert!(progress(&pending).is_none());
        assert!(data(&pending, 0).unwrap().is_empty());
        let public = vec![1_u8, 2, 3];
        let exact = norito::canonical_frame_len(&public).unwrap();
        assert!(encoded(&public, exact - 1).is_err());
        assert_eq!(
            encoded(&public, exact).unwrap(),
            norito::encode_canonical(&public).unwrap()
        );
        assert_eq!(kind(&pending), "pending");
        for outcome in [
            NativePrivateSettlementOutcome::Opened,
            NativePrivateSettlementOutcome::Staged,
            NativePrivateSettlementOutcome::NotPrepared,
        ] {
            assert!(data(&outcome, 0).unwrap().is_empty());
        }
    }
}

//! Charged capture from the concrete applying Time storage checkpoint.
//!
//! This is source custody, not a lifecycle effect or callable authorization.
//! No table lookup, generic source callback or historical-image adapter precedes
//! admission. The Action encoder still owns its separate finite codec budget.

use core::cmp::Ordering;

use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};
use iroha_data_model::{
    block::execution_output::{
        ActionFrameCaptureFailureV1, ActionFrameProductionFailureV1, ActionFrameScanFailureV1,
        BoundedActionFrameScannerV1, RetainedActionFrameBytesV1, TriggerLifecycleActionSemanticsV1,
    },
    query::error::QueryExecutionFail,
};
use mv::storage::StorageReadOnly;
use norito::core::EncodeWorkLimits;

use super::{
    BorrowedTriggerFilter, BorrowedTriggerSourceReadOnly, ExecutableRef, SetTransaction, TriggerId,
    TriggeringEventType,
    contract_relation::{MergeKey, TriggerContractError, Work},
};

/// Cumulative source admission for one enclosing operation; no clone or reset.
/// These are conservative local work units, not gas, allocation or CPU time.
pub struct ApplyingTimeSourceWork {
    work: Work,
    refused: bool,
    #[cfg(test)]
    observed: [usize; 4], // admitted setup, actual next, full key comparison, projection
}

impl ApplyingTimeSourceWork {
    pub(crate) fn new(max_work: u64) -> Self {
        Self {
            work: Work::new(max_work),
            refused: false,
            #[cfg(test)]
            observed: [0; 4],
        }
    }

    fn admit<T>(
        &mut self,
        operation: impl FnOnce(&mut Work) -> Result<T, TriggerContractError>,
    ) -> Result<T, ApplyingTimeSourceError> {
        if self.refused {
            return Err(ApplyingTimeSourceError::ClosedAfterWorkRefusal);
        }
        operation(&mut self.work).map_err(|original| {
            self.refused = true;
            ApplyingTimeSourceError::Work(original)
        })
    }

    #[cfg(test)]
    pub(super) fn observations(&self) -> [usize; 4] {
        self.observed
    }
}

/// Original refusal owners remain inspectable; no formatted diagnostic copy.
#[derive(Debug, thiserror::Error)]
pub enum ApplyingTimeSourceError {
    #[error("applying Time source work refused")]
    Work(#[source] TriggerContractError),
    #[error("applying Time source work is closed after its earlier refusal")]
    ClosedAfterWorkRefusal,
    #[error("applying original trigger ID does not route to the Time table")]
    TimeRouteMismatch,
    #[error("loaded IVM Time source requires a separately admitted original-contract lookup")]
    LoadedIvm,
    #[error("applying Time source ID backing refused")]
    IdBacking(#[source] ChargedBufferError),
    #[error("applying Time source ID fixed backing copy refused")]
    IdCopy(#[source] std::io::Error),
    #[error("applying Time source projection refused")]
    Projection(#[source] QueryExecutionFail),
    #[error("present applying Time row did not yield its original projection")]
    MissingProjection,
    #[error("applying Time Action frame production refused")]
    Frame(ActionFrameProductionFailureV1),
}

/// Move-only snapshot of one actual applying Time row. All variable backing is
/// retained with the original allocation pool. The canonical name bytes and
/// fixed retry tuple are separate from the legacy six-field Action frame.
///
/// The frame is deliberately unvalidated. In particular, capture does not prove
/// inner record validity, authenticated callable schema, mutation ordering,
/// transaction commit, or absence in any of the other trigger tables.
pub struct CapturedApplyingTimeSource {
    id: ChargedBuffer<u8>,
    frame: RetainedActionFrameBytesV1,
    retry: Option<(u32, u64)>,
}

impl CapturedApplyingTimeSource {
    pub(crate) fn id_bytes(&self) -> &[u8] {
        self.id.as_slice()
    }

    pub(crate) fn action_bytes(&self) -> &[u8] {
        self.frame.as_bytes()
    }

    pub(crate) fn retry_state(&self) -> Option<(u32, u64)> {
        self.retry
    }

    pub(crate) fn belongs_to(&self, pool: &AllocationBudget) -> bool {
        self.id.belongs_to(pool) && self.frame.belongs_to(pool)
    }

    /// Consume this exact source and scan its original Action and inner record
    /// wire. No detached inner byte slice or caller-built validation claim is
    /// accepted. This is still separate from authenticated callable semantics.
    pub(crate) fn scan_canonical_arguments<'range>(
        self,
        scanner: &'range mut BoundedActionFrameScannerV1<'_>,
    ) -> Result<CanonicalApplyingTimeSource, ApplyingTimeSourceScanFailure<'range>> {
        let Self { id, frame, retry } = self;
        match scanner.scan_owned_with_canonical_arguments(frame) {
            Ok((frame, semantics)) => Ok(CanonicalApplyingTimeSource {
                source: Self { id, frame, retry },
                semantics,
            }),
            Err(original) => Err(ApplyingTimeSourceScanFailure {
                id,
                retry,
                original,
            }),
        }
    }
}

/// Actual applying source plus supported canonical Action/argument wire checks.
/// No callable schema, registration authorization or lifecycle effect is implied.
pub struct CanonicalApplyingTimeSource {
    source: CapturedApplyingTimeSource,
    semantics: TriggerLifecycleActionSemanticsV1,
}

impl CanonicalApplyingTimeSource {
    pub(crate) fn source(&self) -> &CapturedApplyingTimeSource {
        &self.source
    }

    pub(crate) fn semantics(&self) -> &TriggerLifecycleActionSemanticsV1 {
        &self.semantics
    }
}

/// A rejected scan keeps the same ID, retry tuple, Action backing and borrowed
/// first original scanner error. No refund/recapture or reconstructed error.
pub struct ApplyingTimeSourceScanFailure<'range> {
    id: ChargedBuffer<u8>,
    retry: Option<(u32, u64)>,
    original: ActionFrameCaptureFailureV1<'range>,
}

impl<'range> ApplyingTimeSourceScanFailure<'range> {
    pub(crate) fn failure(&self) -> &'range ActionFrameScanFailureV1 {
        self.original.failure()
    }

    pub(crate) fn id_bytes(&self) -> &[u8] {
        self.id.as_slice()
    }

    pub(crate) fn retry_state(&self) -> Option<(u32, u64)> {
        self.retry
    }

    pub(crate) fn frame(&self) -> Option<&RetainedActionFrameBytesV1> {
        self.original.frame()
    }

    /// Recover the unchanged unvalidated source for disposal or later custody.
    /// This gives no permission to reset or retry the failed scanner range.
    pub(crate) fn into_source(self) -> Option<CapturedApplyingTimeSource> {
        let Self {
            id,
            retry,
            original,
        } = self;
        original
            .into_frame()
            .map(|frame| CapturedApplyingTimeSource { id, frame, retry })
    }
}

impl SetTransaction<'_> {
    /// Check the separate applying ID routing entry before a generic Set mutator.
    /// Charges cover this original bounded scan only, not later instruction work.
    pub(crate) fn require_applying_time_route(
        &self,
        wanted: &TriggerId,
        work: &mut ApplyingTimeSourceWork,
    ) -> Result<(), ApplyingTimeSourceError> {
        work.admit(Work::setup)?;
        let mut rows = self.ids.iter();
        loop {
            work.admit(|work| work.prepay(2))?;
            let Some((id, event_type)) = work.admit(|work| work.next(&mut rows))? else {
                return Err(ApplyingTimeSourceError::TimeRouteMismatch);
            };
            match work.admit(|work| id.compare(wanted, work))? {
                Ordering::Less => continue,
                Ordering::Greater => return Err(ApplyingTimeSourceError::TimeRouteMismatch),
                Ordering::Equal => {
                    work.admit(|work| work.prepay(2))?;
                    return if matches!(event_type, TriggeringEventType::Time) {
                        Ok(())
                    } else {
                        Err(ApplyingTimeSourceError::TimeRouteMismatch)
                    };
                }
            }
        }
    }

    /// Capture one original row from this transaction's current typed Time
    /// checkpoint. The transaction remains borrowed through the complete copy.
    /// `None` means only that this particular Time table has no matching row.
    ///
    /// Cursor setup, terminal next, full ID operands, ID copy, and fixed source
    /// projection are admitted before they occur. `get`, `range`, touched-key
    /// adapters and the generic four-table visitor are intentionally not used.
    pub(crate) fn capture_applying_time_source(
        &self,
        wanted: &TriggerId,
        work: &mut ApplyingTimeSourceWork,
        pool: &AllocationBudget,
        encode_limits: EncodeWorkLimits,
    ) -> Result<Option<CapturedApplyingTimeSource>, ApplyingTimeSourceError> {
        work.admit(Work::setup)?;
        #[cfg(test)]
        {
            work.observed[0] += 1;
        }
        // Concrete StorageTransaction::iter borrows current().iter directly;
        // it does not search for wanted or merge an undo image internally.
        let mut rows = self.time_triggers.iter();
        loop {
            work.admit(|work| work.prepay(2))?; // loop and result branch
            let row = work.admit(|work| work.next(&mut rows))?;
            #[cfg(test)]
            {
                work.observed[1] += 1;
            }
            let Some((id, action)) = row else {
                return Ok(None);
            };
            let order = work.admit(|work| id.compare(wanted, work))?;
            #[cfg(test)]
            {
                work.observed[2] += 1;
            }
            match order {
                Ordering::Less => continue,
                Ordering::Greater => return Ok(None),
                Ordering::Equal => {}
            }

            // Conservative fixed projection capsule: discriminate executable,
            // borrow fixed fields, construct the six-field and two-field stack
            // projections, copy retry words and enter the bounded encoder.
            // Variable payload visits remain under EncodeWorkLimits below.
            work.admit(|work| work.prepay(128))?;
            if matches!(&action.executable, ExecutableRef::Ivm(_)) {
                // Refuse before with_borrowed_trigger_source can look up code.
                return Err(ApplyingTimeSourceError::LoadedIvm);
            }
            work.admit(|work| work.prepay(16))?; // ID borrow/length and backing/copy setup
            let name: &str = id.name().as_ref();
            work.admit(|work| work.bytes(name.len()))?;
            let mut owned_id =
                ChargedBuffer::new(name.len(), pool).map_err(ApplyingTimeSourceError::IdBacking)?;
            owned_id
                .append(name.as_bytes())
                .map_err(ApplyingTimeSourceError::IdCopy)?;
            #[cfg(test)]
            {
                work.observed[3] += 1;
            }
            // Nested Result preserves production refusal owners without coercing
            // them to a query diagnostic or constructing an uncharged String.
            return self
                .with_borrowed_trigger_source(
                    id,
                    BorrowedTriggerFilter::Time(&action.filter),
                    action,
                    |source| {
                        Ok(match source {
                            Some(source) => source
                                .try_prepare_bounded_time_frame(pool, encode_limits)
                                .map(|frame| {
                                    Some(CapturedApplyingTimeSource {
                                        id: owned_id,
                                        frame,
                                        retry: source.retry_state(),
                                    })
                                })
                                .map_err(ApplyingTimeSourceError::Frame),
                            None => Err(ApplyingTimeSourceError::MissingProjection),
                        })
                    },
                )
                .map_err(ApplyingTimeSourceError::Projection)?;
        }
    }
}

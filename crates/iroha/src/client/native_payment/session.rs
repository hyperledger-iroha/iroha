//! Shared original authenticated prefix for bounded native payment observation.

use super::*;
use std::{num::NonZeroUsize, sync::MutexGuard};

/// A bounded authenticated prefix shared by native payments on one selected Global root.
///
/// Clones share the exact original verifier; they do not clone its history or select a
/// new trust root. Each payment retains its own request, journal, fee authorization and
/// proof-read allowance. Concurrent proof walks are serialized and cancellation keeps
/// every successfully authenticated decision. No proof blobs are retained.
///
/// Old warm decisions retire under the explicit local capacity. Each operation's mandatory
/// caller-key custody independently retains its pending cursor and exact successful receipt
/// evidence, so shared-cache pruning cannot retire an unresolved payment obligation.
#[derive(Clone)]
pub struct NativePaymentSession {
    pub(super) owner: Arc<SessionOwner>,
}

pub(super) struct SessionOwner {
    pub(super) verifier: Mutex<SumeragiFinalityVerifier>,
    pub(super) catch_up: Arc<tokio::sync::Mutex<()>>,
    pub(super) retained_decisions: usize,
}

impl core::fmt::Debug for NativePaymentSession {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NativePaymentSession")
            .field("retained_decisions", &self.owner.retained_decisions)
            .finish_non_exhaustive()
    }
}

impl NativePaymentSession {
    /// Retain an independently selected verifier with a finite local history capacity.
    ///
    /// The capacity includes the tip and its witness parents. At least three decisions
    /// are required by the existing finality verifier; historical witness verification
    /// needs its retained decision and parent. This local bound is independent of each
    /// call's proof-read allowance and never changes ledger validity. Any already
    /// authenticated input history is reduced to this same capacity on construction.
    ///
    /// # Errors
    /// A non-Global trust root or a capacity below three decisions.
    pub fn new(
        mut verifier: SumeragiFinalityVerifier,
        retained_decisions: NonZeroUsize,
    ) -> Result<Self, NativePaymentError> {
        if verifier.root_scope() != SumeragiRootScope::Global {
            return Err(NativePaymentError::InvalidRequest(
                "native payment session requires an independently selected Global trust root",
            ));
        }
        if retained_decisions.get() < 3 {
            return Err(NativePaymentError::InvalidRequest(
                "native payment session requires at least three retained decisions",
            ));
        }
        verifier.retain_recent_decisions(retained_decisions.get());
        Ok(Self {
            owner: Arc::new(SessionOwner {
                verifier: Mutex::new(verifier),
                catch_up: Arc::new(tokio::sync::Mutex::new(())),
                retained_decisions: retained_decisions.get(),
            }),
        })
    }

    pub(super) fn verifier(
        &self,
    ) -> Result<MutexGuard<'_, SumeragiFinalityVerifier>, NativePaymentObservationError> {
        self.owner
            .verifier
            .lock()
            .map_err(|_| NativePaymentObservationError::Synchronization)
    }
}

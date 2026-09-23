//! Exact checked fair-ingress custody for the inactive Native runner path.
//!
//! A checked dequeue transfers one physical occurrence here without cloning
//! its payload or dropping its authenticated source and route history. Native
//! driver backpressure returns the same owned input and provenance. Production
//! ingress remains closed until the funded Validate-to-Apply cutover retires the
//! old lane scheduler and installs one process-lived Native owner.
//! The process owner holds one Retry carrier and fail-stops if checked dequeue
//! changes its ownership. TODO: wire this owner through live startup and durable
//! restart reconstruction before opening public Native ingress; the complete
//! funded Validate-to-Apply and publication cutover remains required.

use std::sync::Arc;

use crate::sumeragi::{
    FairV2Ingress, FairV2IngressOwnershipEvidence, InboundBlockMessage, NetworkReplyRoutes,
    message::BlockMessage,
    output_guard::ConsensusOutputGuard,
    v2_lane_driver::{NativeLaneAdmission, NativeLaneInput},
};
use iroha_model_base::peer::PeerId;

#[derive(Debug)]
struct NativeIngressProvenance {
    sender: PeerId,
    via: PeerId,
    reply_routes: Option<NetworkReplyRoutes>,
    ownership: FairV2IngressOwnershipEvidence,
}

/// One original checked Native queue occurrence, retained across driver retry.
#[derive(Debug)]
pub(crate) struct NativeFairIngressCarrier {
    input: NativeLaneInput,
    provenance: NativeIngressProvenance,
}

/// The exact Native driver result together with any still-owned input.
#[derive(Debug)]
pub(crate) enum NativeFairIngressAdmission {
    /// The authenticated driver took the original input and released fair custody.
    Accepted,
    /// The original input and physical provenance await a changing dependency.
    Retry(NativeFairIngressCarrier),
    /// The invalid input remains available to the caller for terminal diagnostics.
    Rejected {
        /// Exact rejected occurrence; dropping it settles local physical custody.
        carrier: NativeFairIngressCarrier,
        /// Cryptographic or current-instance rejection supplied by the driver.
        reason: String,
    },
}

/// Result of servicing at most one process-owned Native fair occurrence.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum NativeFairIngressServiceOutcome {
    /// No Native occurrence is currently admitted to fair ingress.
    Empty,
    /// The authenticated driver took the original input.
    Accepted,
    /// One original occurrence remains in the bounded process owner.
    Retained,
    /// The driver rejected an invalid occurrence and released its local custody.
    Rejected(String),
}

/// Process-lifetime one-slot owner across global height and driver backpressure.
pub(crate) struct NativeFairIngressPump {
    pending: Option<NativeFairIngressCarrier>,
    guard: Arc<ConsensusOutputGuard>,
}

struct NativeIngressTransferFailStop {
    guard: Arc<ConsensusOutputGuard>,
    completed: bool,
}

impl NativeIngressTransferFailStop {
    fn complete(mut self) {
        self.completed = true;
    }
}

impl Drop for NativeIngressTransferFailStop {
    fn drop(&mut self) {
        if !self.completed {
            self.guard.close_admission_for_restart();
        }
    }
}

impl NativeFairIngressPump {
    /// Bind the single retry slot to this runner's consensus fail-stop guard.
    pub(crate) fn new(guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            pending: None,
            guard,
        }
    }

    fn accept_checked_dequeue(&mut self, inbound: InboundBlockMessage) -> Result<(), String> {
        if self.pending.is_some() {
            self.guard.close_admission_for_restart();
            return Err("Native fair-ingress pump already retains an original input".to_owned());
        }
        let carrier = NativeFairIngressCarrier::from_checked_dequeue(inbound).map_err(
            |(_original, error)| {
                self.guard.close_admission_for_restart();
                error
            },
        )?;
        self.pending = Some(carrier);
        Ok(())
    }

    /// Transfer at most one checked occurrence into the exact Native driver.
    /// A retry remains in this process owner and blocks another dequeue until
    /// the driver consumes or rejects the original input.
    pub(crate) fn service_with(
        &mut self,
        ingress: &FairV2Ingress,
        admit: impl FnOnce(NativeLaneInput) -> NativeLaneAdmission,
    ) -> Result<NativeFairIngressServiceOutcome, String> {
        if self.guard.restart_required() {
            return Err("Native fair-ingress output is closed for restart".to_owned());
        }
        let transfer = NativeIngressTransferFailStop {
            guard: Arc::clone(&self.guard),
            completed: false,
        };
        if self.pending.is_none() {
            let inbound = ingress
                .try_recv_if_checked(|candidate| candidate.message().is_native_lane())
                .map_err(|error| {
                    self.guard.close_admission_for_restart();
                    error
                })?;
            let Some(inbound) = inbound else {
                transfer.complete();
                return Ok(NativeFairIngressServiceOutcome::Empty);
            };
            self.accept_checked_dequeue(inbound)?;
        }
        let carrier = self
            .pending
            .take()
            .expect("checked Native dequeue installed the sole pending owner");
        let outcome = match carrier.admit_with(admit) {
            NativeFairIngressAdmission::Accepted => Ok(NativeFairIngressServiceOutcome::Accepted),
            NativeFairIngressAdmission::Retry(carrier) => {
                self.pending = Some(carrier);
                Ok(NativeFairIngressServiceOutcome::Retained)
            }
            NativeFairIngressAdmission::Rejected { carrier, reason } => {
                drop(carrier);
                Ok(NativeFairIngressServiceOutcome::Rejected(reason))
            }
        };
        transfer.complete();
        outcome
    }

    /// Inspect the exact retained occurrence without transferring it.
    #[cfg(test)]
    pub(crate) fn pending_ownership_for_test(&self) -> Option<&FairV2IngressOwnershipEvidence> {
        self.pending
            .as_ref()
            .map(NativeFairIngressCarrier::ownership)
    }

    /// Exercise the checked-dequeue fail-stop boundary without mutating a live queue.
    #[cfg(test)]
    pub(crate) fn accept_checked_dequeue_for_test(
        &mut self,
        inbound: InboundBlockMessage,
    ) -> Result<(), String> {
        self.accept_checked_dequeue(inbound)
    }
}

impl Drop for NativeFairIngressPump {
    fn drop(&mut self) {
        if self.pending.is_some() {
            self.guard.close_admission_for_restart();
        }
    }
}

impl NativeFairIngressCarrier {
    /// Convert one checked Native dequeue; return an altered envelope to its caller.
    /// The caller must fail-stop consensus output on an ownership mismatch.
    pub(crate) fn from_checked_dequeue(
        inbound: InboundBlockMessage,
    ) -> Result<Self, (InboundBlockMessage, String)> {
        let exact = inbound.ingress_ownership().is_some_and(|ownership| {
            inbound.message().is_native_lane()
                && ownership.validate_exact()
                && ownership.matches_message(inbound.message())
                && ownership.matches_semantic_origin(inbound.sender())
                && ownership.matches_native_authenticated_hop(inbound.via())
                && ownership.matches_reply_routes(inbound.reply_routes())
                && ownership.leader_wire_token().is_none()
                && ownership.leader_wire_runtime_receipt().is_none()
        });
        if !exact {
            return Err((
                inbound,
                "Native dequeue changed its exact fair-ingress ownership".to_owned(),
            ));
        }
        let InboundBlockMessage {
            message,
            sender,
            via,
            reply_routes,
            ingress_ownership,
        } = inbound;
        let input = match message {
            BlockMessage::NativeLane(envelope) => NativeLaneInput::Control(envelope),
            BlockMessage::NativeLaneDecision(decision) => NativeLaneInput::Decision(*decision),
            _ => unreachable!("checked Native evidence excludes other message kinds"),
        };
        Ok(Self {
            input,
            provenance: NativeIngressProvenance {
                sender,
                via,
                reply_routes,
                ownership: ingress_ownership.expect("validated Native dequeue has fair ownership"),
            },
        })
    }

    /// Borrow the unaltered source history while retaining the input itself.
    pub(crate) fn ownership(&self) -> &FairV2IngressOwnershipEvidence {
        &self.provenance.ownership
    }

    /// Borrow the exact semantic origin carried by this physical occurrence.
    pub(crate) fn sender(&self) -> &PeerId {
        &self.provenance.sender
    }

    /// Borrow the authenticated hop charged by the bounded ingress queue.
    pub(crate) fn via(&self) -> &PeerId {
        &self.provenance.via
    }

    /// Borrow the retained opaque response routes without changing their owner.
    pub(crate) fn reply_routes(&self) -> Option<&NetworkReplyRoutes> {
        self.provenance.reply_routes.as_ref()
    }

    fn admit_with(
        self,
        admit: impl FnOnce(NativeLaneInput) -> NativeLaneAdmission,
    ) -> NativeFairIngressAdmission {
        let Self { input, provenance } = self;
        match admit(input) {
            NativeLaneAdmission::Accepted => NativeFairIngressAdmission::Accepted,
            NativeLaneAdmission::Retry(input) => {
                NativeFairIngressAdmission::Retry(Self { input, provenance })
            }
            NativeLaneAdmission::Rejected { input, reason } => {
                NativeFairIngressAdmission::Rejected {
                    carrier: Self { input, provenance },
                    reason,
                }
            }
        }
    }

    /// Exercise a backpressured driver without opening a second runtime in tests.
    #[cfg(test)]
    pub(crate) fn retry_for_test(self) -> NativeFairIngressAdmission {
        self.admit_with(NativeLaneAdmission::Retry)
    }
}

//! Mutually exclusive authenticated terminal outcomes of one original private payment.

use super::{VerifiedPrivateSettlementAbort, VerifiedPrivateSettlementReceipt};

/// Independently authenticated successful settlement or successful Abort execution.
///
/// Neither branch can be constructed from a decoded public status or receipt alone. Abort
/// closes the original intent without making its inputs unconditionally spendable elsewhere.
#[derive(Clone, Debug)]
pub enum VerifiedPrivateSettlementOutcome {
    /// The original payment's exact monetary effects were successfully finalized.
    Settled(VerifiedPrivateSettlementReceipt),
    /// An original successful Abort permanently excludes later settlement of this intent.
    Aborted(VerifiedPrivateSettlementAbort),
}

impl VerifiedPrivateSettlementOutcome {
    /// Borrow successful monetary execution, preserving an Abort as a distinct outcome.
    #[must_use]
    pub const fn settled_receipt(&self) -> Option<&VerifiedPrivateSettlementReceipt> {
        match self {
            Self::Settled(receipt) => Some(receipt),
            Self::Aborted(_) => None,
        }
    }

    /// Borrow successful Abort execution, preserving settled money as a distinct outcome.
    #[must_use]
    pub const fn abort_receipt(&self) -> Option<&VerifiedPrivateSettlementAbort> {
        match self {
            Self::Aborted(receipt) => Some(receipt),
            Self::Settled(_) => None,
        }
    }
}

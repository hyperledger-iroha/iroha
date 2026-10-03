//! UNLINKED DRAFT: typed model entry points for the shared fixed-V1 counted writer.
//!
//! TODO: Stage with wire_v1.rs, its type re-export and replacement of the current ordinary
//! encode_default_layout_versioned helper. Do not retain two encoding implementations.

use super::{SignedTransaction, TransactionEntrypoint, wire_v1::WireV1Plan};

impl SignedTransaction {
    /// Measure the exact original signed transaction for a caller-owned output destination.
    ///
    /// The returned plan borrows this transaction. Its extent does not account for serializer
    /// scratch, and it grants no custody, signature or submission authority.
    ///
    /// # Errors
    /// Returns the original serialization error from the real counting pass.
    pub fn wire_plan_v1(&self) -> Result<WireV1Plan<'_>, norito::Error> {
        WireV1Plan::new(self)
    }
}

impl TransactionEntrypoint {
    /// Measure the exact V1 entrypoint, retaining its variant and complete signed authorization.
    ///
    /// This is the versioned entrypoint wire, distinct from both SignedTransaction's wire and
    /// a header-framed native entrypoint. Its extent does not fund serializer scratch.
    ///
    /// # Errors
    /// Returns the original serialization error from the real counting pass.
    pub fn wire_plan_v1(&self) -> Result<WireV1Plan<'_>, norito::Error> {
        WireV1Plan::new(self)
    }
}

use super::*;
use crate::offline::{OfflineToOnlineTransfer, OfflineVerdictRevocation, OfflineWalletCertificate};

isi! {
    /// Register an operator-issued offline allowance certificate on-ledger.
    pub struct RegisterOfflineAllowance {
        /// Certificate describing the allowance commitment and policy.
        pub certificate: OfflineWalletCertificate,
    }
}

isi! {
    /// Submit a bundled offline-to-online transfer proof for settlement.
    pub struct SubmitOfflineToOnlineTransfer {
        /// Aggregated receipts, platform proofs, and balance commitment deltas to reconcile.
        pub transfer: OfflineToOnlineTransfer,
    }
}

isi! {
    /// Register a revoked attestation verdict so POS clients can sync deny lists.
    pub struct RegisterOfflineVerdictRevocation {
        /// Revocation payload describing the verdict id and metadata.
        pub revocation: OfflineVerdictRevocation,
    }
}

impl crate::seal::Instruction for RegisterOfflineAllowance {}
impl crate::seal::Instruction for SubmitOfflineToOnlineTransfer {}
impl crate::seal::Instruction for RegisterOfflineVerdictRevocation {}

impl SubmitOfflineToOnlineTransfer {
    /// Construct a submission instruction from a prepared transfer bundle.
    #[must_use]
    pub fn new(transfer: OfflineToOnlineTransfer) -> Self {
        Self { transfer }
    }
}

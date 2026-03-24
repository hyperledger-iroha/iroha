use super::*;
use crate::offline::{OfflineToOnlineTransfer, OfflineVerdictRevocation, OfflineWalletCertificate};
use crate::{asset::AssetId, prelude::Numeric};
use iroha_crypto::Hash;

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

isi! {
    /// Reclaim remaining allowance from an expired offline certificate back to the controller.
    pub struct ReclaimExpiredOfflineAllowance {
        /// Deterministic identifier of the allowance certificate to reclaim.
        pub certificate_id: Hash,
    }
}

isi! {
    /// Move online balance from the controller account into the configured offline escrow.
    pub struct ReserveOfflineEscrowBalance {
        /// Controller-owned asset to reserve into offline escrow.
        pub asset: AssetId,
        /// Amount to move into offline escrow.
        pub amount: Numeric,
    }
}

isi! {
    /// Move balance from the configured offline escrow back to the controller account.
    pub struct RefundOfflineEscrowBalance {
        /// Controller-owned asset to credit from offline escrow.
        pub asset: AssetId,
        /// Amount to return from offline escrow.
        pub amount: Numeric,
    }
}

impl crate::seal::Instruction for RegisterOfflineAllowance {}
impl crate::seal::Instruction for SubmitOfflineToOnlineTransfer {}
impl crate::seal::Instruction for RegisterOfflineVerdictRevocation {}
impl crate::seal::Instruction for ReclaimExpiredOfflineAllowance {}
impl crate::seal::Instruction for ReserveOfflineEscrowBalance {}
impl crate::seal::Instruction for RefundOfflineEscrowBalance {}

impl SubmitOfflineToOnlineTransfer {
    /// Construct a submission instruction from a prepared transfer bundle.
    #[must_use]
    pub fn new(transfer: OfflineToOnlineTransfer) -> Self {
        Self { transfer }
    }
}

impl ReclaimExpiredOfflineAllowance {
    /// Construct a reclaim instruction for the supplied certificate identifier.
    #[must_use]
    pub fn new(certificate_id: Hash) -> Self {
        Self { certificate_id }
    }
}

impl ReserveOfflineEscrowBalance {
    /// Construct a reserve-to-escrow instruction.
    #[must_use]
    pub fn new(asset: AssetId, amount: Numeric) -> Self {
        Self { asset, amount }
    }
}

impl RefundOfflineEscrowBalance {
    /// Construct an escrow refund instruction.
    #[must_use]
    pub fn new(asset: AssetId, amount: Numeric) -> Self {
        Self { asset, amount }
    }
}

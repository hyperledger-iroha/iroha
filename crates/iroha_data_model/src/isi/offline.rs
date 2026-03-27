use super::*;
use crate::offline::{
    OfflineLineageOperationResult, OfflineLineageRecord, OfflineToOnlineTransfer,
    OfflineVerdictRevocation, OfflineWalletCertificate,
};
use crate::{asset::AssetId, prelude::Numeric};
use iroha_crypto::Hash;

isi! {
    /// Register a zero-balance offline lineage in shared Iroha state.
    pub struct RegisterOfflineLineage {
        /// Shared lineage record to persist.
        pub lineage: OfflineLineageRecord,
    }
}

isi! {
    /// Commit a lineage mutation and its replay result into shared Iroha state.
    pub struct CommitOfflineLineageOperation {
        /// Expected authoritative server revision before applying the mutation.
        pub expected_server_revision: u64,
        /// Expected authoritative state hash before applying the mutation.
        pub expected_state_hash: String,
        /// Updated shared lineage record to persist.
        pub lineage: OfflineLineageRecord,
        /// Replayable result snapshot keyed by `kind:operation_id`.
        pub result: OfflineLineageOperationResult,
    }
}

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
    pub struct LoadOfflineEscrowBalance {
        /// Controller-owned asset to load into offline escrow.
        pub asset: AssetId,
        /// Amount to move into offline escrow.
        pub amount: Numeric,
    }
}

isi! {
    /// Move balance from the configured offline escrow back to the controller account.
    pub struct RedeemOfflineEscrowBalance {
        /// Controller-owned asset to credit from offline escrow.
        pub asset: AssetId,
        /// Amount to return from offline escrow.
        pub amount: Numeric,
    }
}

impl crate::seal::Instruction for RegisterOfflineLineage {}
impl crate::seal::Instruction for CommitOfflineLineageOperation {}
impl crate::seal::Instruction for RegisterOfflineAllowance {}
impl crate::seal::Instruction for SubmitOfflineToOnlineTransfer {}
impl crate::seal::Instruction for RegisterOfflineVerdictRevocation {}
impl crate::seal::Instruction for ReclaimExpiredOfflineAllowance {}
impl crate::seal::Instruction for LoadOfflineEscrowBalance {}
impl crate::seal::Instruction for RedeemOfflineEscrowBalance {}

impl RegisterOfflineLineage {
    /// Construct a lineage-registration instruction for the supplied lineage record.
    #[must_use]
    pub fn new(lineage: OfflineLineageRecord) -> Self {
        Self { lineage }
    }
}

impl CommitOfflineLineageOperation {
    /// Construct a lineage-mutation instruction for the supplied authoritative result.
    #[must_use]
    pub fn new(
        expected_server_revision: u64,
        expected_state_hash: String,
        lineage: OfflineLineageRecord,
        result: OfflineLineageOperationResult,
    ) -> Self {
        Self {
            expected_server_revision,
            expected_state_hash,
            lineage,
            result,
        }
    }
}

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

impl LoadOfflineEscrowBalance {
    /// Construct a load-to-escrow instruction.
    #[must_use]
    pub fn new(asset: AssetId, amount: Numeric) -> Self {
        Self { asset, amount }
    }
}

impl RedeemOfflineEscrowBalance {
    /// Construct an escrow redeem instruction.
    #[must_use]
    pub fn new(asset: AssetId, amount: Numeric) -> Self {
        Self { asset, amount }
    }
}

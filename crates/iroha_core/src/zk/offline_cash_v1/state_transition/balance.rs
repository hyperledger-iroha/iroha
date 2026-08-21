//! Move-only singleton balance owner.

use super::*;
use zeroize::Zeroize as _;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct BalanceSnapshotV1 {
    context_digest: Digest,
    pub(super) wallet_binding: Digest,
    pub(super) guard_device_id: Digest,
    pub(super) hardware_policy_id: Digest,
    pub(super) amount: u128,
    pub(super) head: Digest,
    pub(super) lineage_digest: Digest,
    pub(super) guard_sequence: u64,
    /// Software cache of the hardware-authoritative receive intent.
    pub(super) active_request: Option<Digest>,
}

impl BalanceSnapshotV1 {
    pub(super) fn capture(balance: &BalanceOwnerV1) -> Self {
        Self {
            context_digest: balance.context.digest,
            wallet_binding: balance.wallet_binding,
            guard_device_id: balance.guard_device_id,
            hardware_policy_id: balance.hardware_policy_id,
            amount: balance.amount,
            head: balance.head,
            lineage_digest: balance.lineage_digest,
            guard_sequence: balance.guard_sequence,
            active_request: balance.active_request,
        }
    }

    pub(super) fn matches(&self, balance: &BalanceOwnerV1) -> bool {
        self == &Self::capture(balance)
    }
}

impl Drop for BalanceSnapshotV1 {
    fn drop(&mut self) {
        self.context_digest.zeroize();
        self.wallet_binding.zeroize();
        self.guard_device_id.zeroize();
        self.hardware_policy_id.zeroize();
        self.amount.zeroize();
        self.head.zeroize();
        self.lineage_digest.zeroize();
        self.guard_sequence.zeroize();
        if let Some(request) = self.active_request.as_mut() {
            request.zeroize();
        }
        self.active_request = None;
    }
}

pub(super) fn balance_head(
    context: &OfflineCashStateContextV1,
    wallet_binding: Digest,
    guard_device_id: Digest,
    hardware_policy_id: Digest,
    guard_sequence: u64,
    lineage_digest: Digest,
    amount: u128,
    opening: &[u8; 32],
) -> Digest {
    offline_cash_balance_head_v1(
        &context.digest,
        &wallet_binding,
        &guard_device_id,
        &hardware_policy_id,
        guard_sequence,
        &lineage_digest,
        amount,
        opening,
    )
}

/// Move-only owner of the sole authenticated balance for one wallet/asset.
#[must_use]
pub(crate) struct BalanceOwnerV1 {
    pub(super) context: OfflineCashStateContextV1,
    pub(super) wallet_binding: Digest,
    pub(super) guard_device_id: Digest,
    pub(super) hardware_policy_id: Digest,
    pub(super) amount: u128,
    pub(super) head: Digest,
    pub(super) lineage_digest: Digest,
    pub(super) opening: Zeroizing<Digest>,
    pub(super) guard_sequence: u64,
    /// Software cache of the hardware-authoritative receive intent.
    pub(super) active_request: Option<Digest>,
}

impl fmt::Debug for BalanceOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BalanceOwnerV1")
            .field("context_digest", &self.context.digest)
            .field("wallet_binding", &self.wallet_binding)
            .field("head", &self.head)
            .field("lineage_digest", &self.lineage_digest)
            .field("guard_sequence", &self.guard_sequence)
            .field("amount", &"[REDACTED]")
            .field("opening", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl Drop for BalanceOwnerV1 {
    fn drop(&mut self) {
        self.wallet_binding.zeroize();
        self.guard_device_id.zeroize();
        self.hardware_policy_id.zeroize();
        self.amount.zeroize();
        self.head.zeroize();
        self.lineage_digest.zeroize();
        self.opening.zeroize();
        self.guard_sequence.zeroize();
        if let Some(request) = self.active_request.as_mut() {
            request.zeroize();
        }
        self.active_request = None;
    }
}

impl BalanceOwnerV1 {
    /// Privileged restoration boundary used only after authenticated durable-state recovery.
    ///
    /// `active_request` is not authoritative. Before this owner can issue a new
    /// intent, recovery must reconcile it with the sealed hardware journal via
    /// `recover_pending_v1`; a mismatch fails closed.
    pub(super) fn restore_authenticated(
        context: OfflineCashStateContextV1,
        wallet_binding: Digest,
        guard_device_id: Digest,
        hardware_policy_id: Digest,
        amount: u128,
        opening: Zeroizing<Digest>,
        lineage_digest: Digest,
        guard_sequence: u64,
        active_request: Option<Digest>,
    ) -> Result<Self, StateTransitionErrorV1> {
        if wallet_binding == [0; 32]
            || guard_device_id == [0; 32]
            || hardware_policy_id == [0; 32]
            || opening.iter().all(|byte| *byte == 0)
            || lineage_digest == [0; 32]
            || active_request == Some([0; 32])
        {
            return Err(StateTransitionErrorV1::InvalidContext);
        }
        let head = balance_head(
            &context,
            wallet_binding,
            guard_device_id,
            hardware_policy_id,
            guard_sequence,
            lineage_digest,
            amount,
            &opening,
        );
        Ok(Self {
            context,
            wallet_binding,
            guard_device_id,
            hardware_policy_id,
            amount,
            head,
            lineage_digest,
            opening,
            guard_sequence,
            active_request,
        })
    }

    /// Current private amount, exposed only inside Core.
    pub(crate) const fn amount(&self) -> u128 {
        self.amount
    }

    /// Current authenticated balance commitment.
    pub(crate) const fn head(&self) -> Digest {
        self.head
    }

    /// Non-circular lineage anchor committed by this balance head.
    pub(crate) const fn lineage_digest(&self) -> Digest {
        self.lineage_digest
    }

    /// Current exact hardware sequence.
    pub(crate) const fn guard_sequence(&self) -> u64 {
        self.guard_sequence
    }

    /// Cached sole active signed-request digest, when present.
    pub(crate) const fn active_request(&self) -> Option<Digest> {
        self.active_request
    }
}

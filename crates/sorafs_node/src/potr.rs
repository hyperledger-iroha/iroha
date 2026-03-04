//! PoTR receipt tracking for the embedded storage node.

use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
};

use sorafs_manifest::{potr::PotrReceiptV1, proof_stream::ProofStreamTier};

/// Maximum number of PoTR receipts retained in memory.
const MAX_TRACKED_RECEIPTS: usize = 1024;

/// Tracks recent PoTR receipts for streaming and diagnostics.
#[derive(Debug, Default, Clone)]
pub struct PotrTracker {
    inner: Arc<RwLock<VecDeque<PotrReceiptV1>>>,
}

impl PotrTracker {
    /// Records a new PoTR receipt.
    pub fn record_receipt(&self, receipt: PotrReceiptV1) {
        let mut deque = self.inner.write().expect("potr tracker poisoned");
        if deque.len() >= MAX_TRACKED_RECEIPTS {
            deque.pop_front();
        }
        deque.push_back(receipt);
    }

    /// Returns receipts matching the given manifest/provider/tier filters.
    pub fn receipts_for(
        &self,
        manifest_digest: &[u8; 32],
        provider_id: &[u8; 32],
        tier: Option<ProofStreamTier>,
    ) -> Vec<PotrReceiptV1> {
        let deque = self.inner.read().expect("potr tracker poisoned");
        deque
            .iter()
            .filter(|receipt| {
                receipt.manifest_digest == *manifest_digest
                    && receipt.provider_id == *provider_id
                    && tier.is_none_or(|filter| receipt.tier == filter)
            })
            .cloned()
            .collect()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.read().expect("potr tracker poisoned").len()
    }
}

#[cfg(test)]
mod tests {
    use sorafs_manifest::potr::{POTR_RECEIPT_VERSION_V1, PotrReceiptValidationError, PotrStatus};

    use super::*;

    fn sample_receipt(tier: ProofStreamTier) -> PotrReceiptV1 {
        PotrReceiptV1 {
            version: POTR_RECEIPT_VERSION_V1,
            manifest_digest: [0x11; 32],
            provider_id: [0x22; 32],
            tier,
            deadline_ms: 90_000,
            latency_ms: 45_000,
            status: PotrStatus::Success,
            requested_at_ms: 1_700_000_000_000,
            responded_at_ms: 1_700_000_045_000,
            recorded_at_ms: 1_700_000_045_100,
            range_start: 0,
            range_end: 65_535,
            request_id: Some([0x55; 16]),
            trace_id: None,
            note: None,
            gateway_signature: None,
            provider_signature: None,
        }
    }

    #[test]
    fn tracker_retains_bounded_history() {
        let tracker = PotrTracker::default();
        for index in 0..(MAX_TRACKED_RECEIPTS + 10) {
            let mut receipt = sample_receipt(ProofStreamTier::Hot);
            receipt.latency_ms = index as u32;
            tracker.record_receipt(receipt);
        }
        assert_eq!(tracker.len(), MAX_TRACKED_RECEIPTS);
    }

    #[test]
    fn tracker_filters_by_manifest_provider_and_tier() {
        let tracker = PotrTracker::default();
        let mut receipt_hot = sample_receipt(ProofStreamTier::Hot);
        receipt_hot.latency_ms = 10;
        tracker.record_receipt(receipt_hot.clone());

        let mut receipt_warm = sample_receipt(ProofStreamTier::Warm);
        receipt_warm.latency_ms = 20;
        tracker.record_receipt(receipt_warm.clone());

        let results = tracker.receipts_for(
            &receipt_hot.manifest_digest,
            &receipt_hot.provider_id,
            Some(ProofStreamTier::Hot),
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].latency_ms, 10);

        let results = tracker.receipts_for(
            &receipt_hot.manifest_digest,
            &receipt_hot.provider_id,
            Some(ProofStreamTier::Warm),
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].latency_ms, 20);
    }

    #[test]
    fn tracker_accepts_validated_receipts() {
        let tracker = PotrTracker::default();
        let receipt = sample_receipt(ProofStreamTier::Hot);
        tracker.record_receipt(receipt.clone());
        let filtered = tracker.receipts_for(&receipt.manifest_digest, &receipt.provider_id, None);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn tracker_rejects_invalid_receipts_via_validation() {
        let tracker = PotrTracker::default();
        let mut receipt = sample_receipt(ProofStreamTier::Hot);
        receipt.deadline_ms = 0;
        assert_eq!(
            receipt.validate(),
            Err(PotrReceiptValidationError::ZeroDeadline)
        );
        // Ensure the invalid receipt is not recorded.
        if receipt.validate().is_ok() {
            tracker.record_receipt(receipt);
        }
        assert_eq!(tracker.len(), 0);
    }
}

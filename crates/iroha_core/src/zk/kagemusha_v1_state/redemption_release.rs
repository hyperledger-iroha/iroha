//! Closed settlement authorization for an indexed KAGEMUSHA redemption outbox entry.
//!
//! A Torii operation status is much larger than the native sender command budget because it
//! carries complete consensus finality and an ordinary-write membership witness. Core verifies
//! that full object against a caller-pinned trust anchor, binds it to the exact installed
//! redemption voucher, and then emits the compact public projection below. The projection is
//! useful as hardware selector material; only the non-serializable verified capability can
//! authorize release of the durable outbox entry.

use iroha_data_model::{
    NetworkId,
    block::consensus_v2::HeightContextId,
    isi::kagemusha_v1::{
        KagemushaFinalityTrustAnchorV1, KagemushaOperationKindV1 as ChainOperationKindV1,
        KagemushaOperationResultV1, KagemushaOperationStateV1, KagemushaOperationStatusV1,
        KagemushaRedemptionResultV1,
    },
    kagemusha::{KagemushaOperationKindV1, KagemushaRedemptionVoucherV1},
};
use norito::codec::{Decode, Encode};

use super::{
    DigestV1, DurableOutgoingEnvelopeV1, KAGEMUSHA_STATE_VERSION_V1, KagemushaOutgoingEnvelopeV1,
    KagemushaOutgoingOperationPhaseV1, KagemushaOutgoingOperationRecordV1,
    KagemushaOutgoingPublicInputsV1, KagemushaStateErrorV1, canonical_sha256_digest,
};

/// Domain of the compact, Core-authenticated redemption terminal receipt.
pub const KAGEMUSHA_REDEMPTION_TERMINAL_RECEIPT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:device:v1:redemption-terminal-receipt";

const KAGEMUSHA_AUTHENTICATED_REDEMPTION_STATUS_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:device:v1:authenticated-redemption-status";

/// Compact public projection of one fully authenticated redemption settlement.
///
/// This value is safe to pass through the bounded native command ABI, but decoding or hashing it
/// does not grant release authority. A qualified in-process caller must also supply the matching
/// [`VerifiedKagemushaRedemptionReleaseV1`] created by Core from the complete finalized status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.redemption-terminal-receipt")]
pub struct KagemushaRedemptionTerminalReceiptV1 {
    /// Sole first-release layout version.
    pub version: u16,
    /// Caller-pinned network that finalized the settlement.
    pub network_id: NetworkId,
    /// Exact idempotent Torii operation and native operation-index key.
    pub operation_id: DigestV1,
    /// Exact identity of the installed redemption voucher.
    pub redemption_id: DigestV1,
    /// Proof-derived one-use terminal nullifier consumed by consensus.
    pub terminal_nullifier: DigestV1,
    /// Digest of the byte-identical voucher retained in the native outbox.
    pub envelope_digest: DigestV1,
    /// Canonical digest of the reserve receipt proven under the finalized block.
    pub reserve_receipt_digest: DigestV1,
    /// Digest of the complete status after full caller-pinned finality verification.
    pub authenticated_status_digest: DigestV1,
    /// Finalized block height pinned by the caller.
    pub finalized_block_height: u64,
    /// Exact externally authenticated consensus context at that height.
    pub height_context_id: HeightContextId,
}

impl KagemushaRedemptionTerminalReceiptV1 {
    /// Validate the compact selector and finality-anchor shape.
    ///
    /// This is intentionally structural. Only Core's verification of the complete operation
    /// status can construct the closed release capability.
    ///
    /// # Errors
    ///
    /// Returns an error for a wrong version, zero identity, or malformed finality anchor.
    pub fn validate_shape(&self) -> Result<(), KagemushaStateErrorV1> {
        if self.version != KAGEMUSHA_STATE_VERSION_V1
            || self.network_id.as_bytes() == &[0; 32]
            || self.finalized_block_height == 0
            || self
                .height_context_id
                .0
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || [
                self.operation_id,
                self.redemption_id,
                self.terminal_nullifier,
                self.envelope_digest,
                self.reserve_receipt_digest,
                self.authenticated_status_digest,
            ]
            .contains(&[0; 32])
        {
            return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
        }
        Ok(())
    }

    /// Compute the canonical digest consumed by the operation-index tombstone and hardware op 12.
    ///
    /// # Errors
    ///
    /// Returns an error if the projection is malformed or canonical Norito encoding fails.
    pub fn canonical_digest(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.validate_shape()?;
        canonical_sha256_digest(KAGEMUSHA_REDEMPTION_TERMINAL_RECEIPT_DOMAIN_V1, self)
    }
}

/// Non-serializable proof that Core authenticated one exact finalized redemption settlement.
///
/// There is deliberately no public constructor and this type is not `Clone`, `Copy`, `Encode`, or
/// `Decode`. Raw ABI bytes and host-computed digests therefore cannot manufacture release
/// authority. The capability is consumed when the indexed outbox entry is retired.
#[derive(Debug)]
pub struct VerifiedKagemushaRedemptionReleaseV1 {
    terminal_receipt: KagemushaRedemptionTerminalReceiptV1,
    terminal_receipt_digest: DigestV1,
}

impl VerifiedKagemushaRedemptionReleaseV1 {
    /// Borrow the compact public projection to bind a qualified hardware release command.
    #[must_use]
    pub const fn terminal_receipt(&self) -> &KagemushaRedemptionTerminalReceiptV1 {
        &self.terminal_receipt
    }

    /// Return the terminal digest retained by the Core operation-index tombstone.
    #[must_use]
    pub const fn terminal_receipt_digest(&self) -> DigestV1 {
        self.terminal_receipt_digest
    }

    /// Return the exact operation-index key authorized for release.
    #[must_use]
    pub const fn operation_id(&self) -> DigestV1 {
        self.terminal_receipt.operation_id
    }

    /// Return the exact installed voucher identity authorized for release.
    #[must_use]
    pub const fn redemption_id(&self) -> DigestV1 {
        self.terminal_receipt.redemption_id
    }

    /// Return the exact terminal nullifier consumed by consensus.
    #[must_use]
    pub const fn terminal_nullifier(&self) -> DigestV1 {
        self.terminal_receipt.terminal_nullifier
    }

    /// Return the exact durable-envelope digest authorized for release.
    #[must_use]
    pub const fn envelope_digest(&self) -> DigestV1 {
        self.terminal_receipt.envelope_digest
    }

    /// Return the finalized reserve-receipt digest.
    #[must_use]
    pub const fn reserve_receipt_digest(&self) -> DigestV1 {
        self.terminal_receipt.reserve_receipt_digest
    }

    /// Return the caller-pinned finalized height.
    #[must_use]
    pub const fn finalized_block_height(&self) -> u64 {
        self.terminal_receipt.finalized_block_height
    }

    /// Return the caller-pinned consensus height context.
    #[must_use]
    pub const fn height_context_id(&self) -> HeightContextId {
        self.terminal_receipt.height_context_id
    }

    pub(super) fn validate_against_record(
        &self,
        record: &KagemushaOutgoingOperationRecordV1,
        durable: Option<&DurableOutgoingEnvelopeV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        record
            .validate_terminal_release_state()
            .map_err(|_| KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt)?;
        self.terminal_receipt.validate_shape()?;
        if self.terminal_receipt_digest != self.terminal_receipt.canonical_digest()?
            || record.operation_kind != KagemushaOperationKindV1::RedeemSplit
            || record.operation_id != self.operation_id()
            || record.outcome_id != self.redemption_id()
            || record.envelope_digest != Some(self.envelope_digest())
        {
            return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
        }
        match record.phase {
            KagemushaOutgoingOperationPhaseV1::Installed => {
                let durable =
                    durable.ok_or(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt)?;
                validate_installed_record_envelope(record, durable)?;
                let KagemushaOutgoingEnvelopeV1::Redemption(voucher) = &durable.envelope else {
                    return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
                };
                if voucher.statement.redemption_id != self.redemption_id()
                    || voucher.statement.terminal_nullifier != self.terminal_nullifier()
                {
                    return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
                }
                Ok(())
            }
            KagemushaOutgoingOperationPhaseV1::Released
                if durable.is_none()
                    && record.terminal_receipt_digest == Some(self.terminal_receipt_digest) =>
            {
                Ok(())
            }
            _ => Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt),
        }
    }
}

pub(super) fn verify_indexed_redemption_release(
    record: &KagemushaOutgoingOperationRecordV1,
    durable: Option<&DurableOutgoingEnvelopeV1>,
    operation_id: DigestV1,
    status: &KagemushaOperationStatusV1,
    trust_anchor: &KagemushaFinalityTrustAnchorV1,
) -> Result<VerifiedKagemushaRedemptionReleaseV1, KagemushaStateErrorV1> {
    status
        .validate_against(trust_anchor)
        .map_err(|_| KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt)?;
    if status.operation_id != operation_id
        || status.kind != ChainOperationKindV1::Redemption
        || status.state != KagemushaOperationStateV1::Applied
        || status.rejection.is_some()
    {
        return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
    }
    let Some(KagemushaOperationResultV1::Redemption(result)) = status.result.as_ref() else {
        return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
    };
    if result.request.operation_id != operation_id {
        return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
    }

    record
        .validate_terminal_release_state()
        .map_err(|_| KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt)?;
    if record.operation_id != operation_id
        || record.operation_kind != KagemushaOperationKindV1::RedeemSplit
        || record.outcome_id != result.request.voucher.statement.redemption_id
        || record.context.lane.network_id != trust_anchor.network_id
    {
        return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
    }

    let envelope_digest = record
        .envelope_digest
        .ok_or(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt)?;
    match record.phase {
        KagemushaOutgoingOperationPhaseV1::Installed => {
            let durable =
                durable.ok_or(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt)?;
            validate_installed_redemption(record, durable, result)?;
        }
        KagemushaOutgoingOperationPhaseV1::Released if durable.is_none() => {}
        _ => return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt),
    }

    let reserve_receipt_digest = result
        .finality
        .reserve_receipt_witness
        .receipt
        .canonical_digest()
        .map_err(|_| KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt)?;
    let authenticated_status_digest =
        canonical_sha256_digest(KAGEMUSHA_AUTHENTICATED_REDEMPTION_STATUS_DOMAIN_V1, status)?;
    let terminal_receipt = KagemushaRedemptionTerminalReceiptV1 {
        version: KAGEMUSHA_STATE_VERSION_V1,
        network_id: trust_anchor.network_id,
        operation_id,
        redemption_id: result.request.voucher.statement.redemption_id,
        terminal_nullifier: result.request.voucher.statement.terminal_nullifier,
        envelope_digest,
        reserve_receipt_digest,
        authenticated_status_digest,
        finalized_block_height: trust_anchor.block_height,
        height_context_id: trust_anchor.height_context_id,
    };
    let terminal_receipt_digest = terminal_receipt.canonical_digest()?;
    let verified = VerifiedKagemushaRedemptionReleaseV1 {
        terminal_receipt,
        terminal_receipt_digest,
    };
    verified.validate_against_record(record, durable)?;
    Ok(verified)
}

fn validate_installed_redemption(
    record: &KagemushaOutgoingOperationRecordV1,
    durable: &DurableOutgoingEnvelopeV1,
    result: &KagemushaRedemptionResultV1,
) -> Result<(), KagemushaStateErrorV1> {
    validate_installed_record_envelope(record, durable)?;
    let KagemushaOutgoingEnvelopeV1::Redemption(voucher) = &durable.envelope else {
        return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
    };
    if voucher != &result.request.voucher
        || canonical_voucher_bytes(voucher)? != durable.canonical_envelope_bytes
        || record.outcome_id != voucher.statement.redemption_id
    {
        return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
    }
    record
        .context
        .validate_lifecycle(&voucher.statement.lifecycle)
        .map_err(|_| KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt)?;
    let Some(KagemushaOutgoingPublicInputsV1::RedeemSplit {
        amount,
        beneficiary,
    }) = record.inputs.as_ref()
    else {
        return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
    };
    if *amount != voucher.statement.amount || beneficiary != &voucher.statement.beneficiary {
        return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
    }
    Ok(())
}

fn validate_installed_record_envelope(
    record: &KagemushaOutgoingOperationRecordV1,
    durable: &DurableOutgoingEnvelopeV1,
) -> Result<(), KagemushaStateErrorV1> {
    if record.phase != KagemushaOutgoingOperationPhaseV1::Installed
        || record.preparation_id != durable.committed.candidate.prepared.preparation_id
        || record.outbox_reservation_id
            != durable
                .committed
                .candidate
                .prepared
                .outbox_reservation
                .reservation_id
        || record.candidate_digest != Some(durable.committed.candidate.candidate_envelope_digest)
        || record.commit_certificate_digest != Some(durable.committed.commit_certificate_digest)
        || record.envelope_digest != Some(durable.envelope_digest)
    {
        return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
    }
    Ok(())
}

fn canonical_voucher_bytes(
    voucher: &KagemushaRedemptionVoucherV1,
) -> Result<Vec<u8>, KagemushaStateErrorV1> {
    norito::encode_canonical(voucher).map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::consensus_v2::HeightContext;

    use super::*;

    fn receipt() -> KagemushaRedemptionTerminalReceiptV1 {
        KagemushaRedemptionTerminalReceiptV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"kagemusha-redemption-terminal-receipt-network",
            ))),
            operation_id: [0x11; 32],
            redemption_id: [0x12; 32],
            terminal_nullifier: [0x13; 32],
            envelope_digest: [0x14; 32],
            reserve_receipt_digest: [0x15; 32],
            authenticated_status_digest: [0x16; 32],
            finalized_block_height: 7,
            height_context_id: HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
                Hash::new(b"kagemusha-redemption-terminal-receipt-context"),
            )),
        }
    }

    #[test]
    fn exact_terminal_receipt_retry_has_one_digest() {
        let receipt = receipt();
        assert_eq!(receipt.canonical_digest(), receipt.canonical_digest());
    }

    #[test]
    fn conflicting_terminal_receipt_changes_digest() {
        let receipt = receipt();
        let mut conflict = receipt;
        conflict.reserve_receipt_digest[0] ^= 1;
        assert_ne!(
            receipt.canonical_digest().expect("valid receipt"),
            conflict.canonical_digest().expect("valid conflict shape")
        );
    }
}

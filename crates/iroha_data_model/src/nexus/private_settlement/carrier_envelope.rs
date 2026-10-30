//! Canonical transport reservation for the original private-payment continuation.

use super::*;
use crate::{
    isi::{
        InstructionBox,
        private_settlement::{AbortAtomicPrivateSettlementV1, FinalizeAtomicPrivateSettlementV1},
    },
    parameter::system::resource_coherence::MAX_TRANSACTION_SIGNER_BYTES,
    privacy::PRIVATE_SETTLEMENT_MAX_CARRIER_SIGNERS_V1,
    transaction::{SignedTransaction, TransactionBuilder},
};
use std::{num::NonZeroU32, time::Duration};

/// Reserve complete Finalize and Abort envelopes before accepting an original Register.
///
/// The canonical encoder measures the original sponsor's **entire** controller, fee intent,
/// network and metadata, the required sequence/height metadata at their largest values, and
/// fixed V1 Prepare/Commit certificate geometry. The independently measured worst-case signer
/// allowance covers every supported algorithm and up to the native signer bound. It is
/// intentionally conservative: a smaller quorum need not use all reserved bytes.
///
/// Certificate/signature bytes used during sizing are never verified or returned as authority.
/// The sole result is a canonical signed-transaction frame byte count. No key is generated, no signature is produced, and original
/// transaction/QC authentication remains mandatory. Future unrelated metadata/attachments are
/// not required continuation context and do not consume the reservation made here.
///
/// # Errors
/// Returns the canonical encoder failure or checked length overflow.
pub fn private_settlement_required_continuation_bytes_v1(
    original: &SignedTransaction,
    barrier: &PrivateSettlementPrepareBarrierV1,
) -> Result<u64, norito::Error> {
    let invalid =
        || norito::Error::Io(std::io::Error::other("invalid private carrier reservation"));
    if original.network_id() != Some(&barrier.manifest.network_id)
        || original.authority() != &barrier.manifest.sponsor
        || original.fee_payment_intent() != &barrier.manifest.public_fee_intent
        || barrier.validate_shape().is_err()
    {
        return Err(invalid());
    }
    // The aggregate and bitmap are fixed-width for all allowed genuine quorums. Reusing
    // their original bytes avoids creating any new cryptographic material for measurement.
    let bundle = PrivateSettlementCommitBundleV1 {
        version: barrier.version,
        manifest: barrier.manifest.clone(),
        authority_catalog: barrier.authority_catalog.clone(),
        legs: barrier
            .deltas
            .iter()
            .zip(&barrier.prepare_certificates)
            .map(|(delta, prepare)| {
                let mut commit = prepare.clone();
                commit.body.phase = PrivateSettlementPhaseV1::Commit;
                commit.body.prepared_bundle_digest = barrier.prepared_bundle_digest;
                PrivateSettlementLegReceiptV1 {
                    delta: delta.clone(),
                    prepare: prepare.clone(),
                    commit,
                }
            })
            .collect(),
    };
    let instructions: [InstructionBox; 2] = [
        FinalizeAtomicPrivateSettlementV1::new(bundle).into(),
        AbortAtomicPrivateSettlementV1::new(
            barrier.manifest.clone(),
            PrivateSettlementAbortReasonV1::Expired,
        )
        .into(),
    ];
    let mut metadata = original.metadata().clone();
    for name in ["tx_sequence", "expires_at_height"] {
        metadata.insert(
            name.parse().map_err(|_| invalid())?,
            iroha_primitives::json::Json::new(u64::MAX),
        );
    }
    let signer_reserve = MAX_TRANSACTION_SIGNER_BYTES
        .checked_mul(PRIVATE_SETTLEMENT_MAX_CARRIER_SIGNERS_V1 as u64)
        .ok_or_else(invalid)?;
    instructions
        .into_iter()
        .try_fold(0_u64, |maximum, instruction| {
            let mut builder = TransactionBuilder::new(
                barrier.manifest.network_id,
                barrier.manifest.sponsor.clone(),
                barrier.manifest.public_fee_intent.clone(),
            )
            .with_metadata(metadata.clone())
            .with_instructions([instruction]);
            builder.set_nonce(NonZeroU32::MAX);
            builder.set_creation_time(Duration::from_millis(u64::MAX - 1));
            builder.set_ttl(Duration::from_millis(1));
            // This value is a private sizing projection, not a signed continuation. The original
            // signature is reused only to retain its canonical framing. The full signer reserve
            // above covers the future authorization bundle, independently of this original proof.
            let projection = builder.build_with_signature(original.signature().payload().clone());
            let bytes = u64::try_from(
                SignedTransaction::canonical_frame_len_from_wire_len_v1(
                    projection.wire_plan_v1()?.wire_length(),
                )
                .ok_or_else(invalid)?,
            )
            .map_err(|_| invalid())?;
            Ok(maximum.max(bytes.checked_add(signer_reserve).ok_or_else(invalid)?))
        })
}

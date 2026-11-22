//! Standalone G3 receipt data and the bounded result custody of its distinct selected owner.
//!
//! Neither type is reachable from the installed `SignedBlock`/native output graph, and a
//! value alone proves neither execution nor certification. Core's selected G3 result owner
//! admits each receipt into one block's [`RuntimeEnactmentResultsV1`] before it applies the
//! original effect transaction that receipt describes. A root selects that owner only through
//! an exact compiled services profile naming [`RUNTIME_ENACTMENT_RESULT_SERVICE_V1`] with the
//! reference closure [`RUNTIME_ENACTMENT_RESULT_SERVICE_CLOSURE_V1`].
//!
//! TODO(N14): bind the admitted results into the block's committed outputs, then compose the
//! service. Until both exist no compiled services profile names the service, every root
//! answers "not selected", and G3 application keeps refusing before effects.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize,
    governance::{
        GovernanceAnchorV1,
        anonymous_ballot::Digest512,
        types::{GovernanceAttemptId, GovernanceAttemptStatusV1, GovernanceStageV1},
    },
};

/// Name of the services-profile entry that selects the G3 runtime enactment result owner.
pub const RUNTIME_ENACTMENT_RESULT_SERVICE_V1: &str = "parliament_runtime_enactment_result_v1";

/// Most runtime enactment receipts one block's result custody admits. A live enactment
/// beyond it stays due, uncharged, for a later block.
pub const RUNTIME_ENACTMENT_RESULTS_PER_BLOCK_V1: u32 = 16;

/// Most complete canonical frame bytes of one runtime enactment receipt.
pub const RUNTIME_ENACTMENT_RECEIPT_MAX_BYTES_V1: u64 = 1_024;

/// Exact reference closure of the G3 result service: the receipt and custody grammar, both
/// bounds and the admit-before-apply rule. Any change to them is a different service.
pub const RUNTIME_ENACTMENT_RESULT_SERVICE_CLOSURE_V1: &[u8] = b"iroha.governance.parliament.anonymous.v1:RuntimeEnactmentReceiptV1:RuntimeEnactmentResultsV1:per-block-16:receipt-max-1024:admit-before-apply-v1";

/// The exact resulting attempt of a positioned runtime enactment, outside native outputs.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::governance::anonymous_ballot::RuntimeEnactmentReceiptV1")]
pub struct RuntimeEnactmentReceiptV1 {
    /// The shared pass position and committed governance time of the effect.
    pub anchor: GovernanceAnchorV1,
    /// The original attempt whose production certificate authorized consideration.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Enacted, superseded or deterministically failed, as the original reducer recorded.
    pub status: GovernanceAttemptStatusV1,
    /// The resulting stage retained by that same attempt.
    pub stage: GovernanceStageV1,
    /// The existing domain-separated canonical digest of the resulting attempt State.
    pub attempt_state_digest: Digest512,
}

/// A receipt whose shape cannot describe a positioned runtime enactment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeEnactmentReceiptErrorV1 {
    /// Block zero cannot own a positioned governance effect.
    #[error("runtime enactment receipt requires a nonzero block height")]
    ZeroHeight,
    /// Only the original terminal execution outcomes describe this receipt.
    #[error("runtime enactment receipt requires a terminal execution outcome")]
    NonterminalOutcome,
}

impl RuntimeEnactmentReceiptV1 {
    /// Check data shape only; the selected result owner proves the source joins.
    ///
    /// # Errors
    /// Rejects a zero-height anchor or a status other than the three enactment outcomes.
    pub fn validate(&self) -> Result<(), RuntimeEnactmentReceiptErrorV1> {
        if self.anchor.height == 0 {
            return Err(RuntimeEnactmentReceiptErrorV1::ZeroHeight);
        }
        if !matches!(
            self.status,
            GovernanceAttemptStatusV1::Enacted
                | GovernanceAttemptStatusV1::Superseded
                | GovernanceAttemptStatusV1::ExecutionFailed
        ) {
            return Err(RuntimeEnactmentReceiptErrorV1::NonterminalOutcome);
        }
        Ok(())
    }
}

/// Why one block's runtime enactment result custody refused a receipt. The custody is
/// unchanged by every refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeEnactmentResultsErrorV1 {
    /// The node could not allocate room for the receipt: a local refusal, never a verdict.
    #[error("runtime enactment results cannot be allocated")]
    Allocation,
    /// The custody already holds [`RUNTIME_ENACTMENT_RESULTS_PER_BLOCK_V1`] receipts. Its
    /// owner checks [`RuntimeEnactmentResultsV1::fits`] before the item mutates anything, so
    /// this is never a capacity deferral.
    #[error("runtime enactment results exceed their per-block bound")]
    Capacity,
    /// The receipt does not describe a positioned runtime enactment.
    #[error("invalid runtime enactment receipt: {0}")]
    Receipt(RuntimeEnactmentReceiptErrorV1),
    /// The receipt names another block height or governance time than the custody's.
    #[error("a runtime enactment receipt names another block")]
    OtherBlock,
    /// The receipt's pass position does not follow the previous receipt's.
    #[error("runtime enactment receipts require strictly increasing pass positions")]
    Position,
    /// The custody already holds a receipt of the same attempt.
    #[error("a block holds at most one runtime enactment receipt per attempt")]
    DuplicateAttempt,
    /// The receipt's canonical frame cannot be measured or exceeds
    /// [`RUNTIME_ENACTMENT_RECEIPT_MAX_BYTES_V1`].
    #[error("a runtime enactment receipt exceeds its frame cap")]
    Oversized,
}

/// The runtime enactment results of one block: the sole custody the selected G3 result owner
/// admits complete receipts into, in pass order, before it applies the effects they describe.
///
/// The custody is bound to its block's height and governance time. It holds at most
/// [`RUNTIME_ENACTMENT_RESULTS_PER_BLOCK_V1`] receipts, each within
/// [`RUNTIME_ENACTMENT_RECEIPT_MAX_BYTES_V1`], with strictly increasing pass positions and at
/// most one receipt per attempt. The owner asks [`Self::fits`] before an item mutates State
/// and an item that does not fit stays due: admission after a successful `fits` cannot fail
/// for capacity.
///
/// The custody is neither clonable nor serializable: one block has one custody.
#[derive(Debug, PartialEq, Eq)]
pub struct RuntimeEnactmentResultsV1 {
    height: u64,
    timestamp_ms: u64,
    receipts: Vec<RuntimeEnactmentReceiptV1>,
}

impl RuntimeEnactmentResultsV1 {
    /// The empty custody of the block at `height` with governance time `timestamp_ms`.
    #[must_use]
    pub const fn new(height: u64, timestamp_ms: u64) -> Self {
        Self {
            height,
            timestamp_ms,
            receipts: Vec::new(),
        }
    }

    /// The block height every admitted receipt names.
    #[must_use]
    pub const fn height(&self) -> u64 {
        self.height
    }

    /// The governance time every admitted receipt names.
    #[must_use]
    pub const fn timestamp_ms(&self) -> u64 {
        self.timestamp_ms
    }

    /// Whether one more receipt fits the per-block bound.
    #[must_use]
    pub fn fits(&self) -> bool {
        u64::try_from(self.receipts.len()).unwrap_or(u64::MAX)
            < u64::from(RUNTIME_ENACTMENT_RESULTS_PER_BLOCK_V1)
    }

    /// Whether no receipt was admitted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.receipts.is_empty()
    }

    /// The admitted receipts, in pass order.
    #[must_use]
    pub fn receipts(&self) -> &[RuntimeEnactmentReceiptV1] {
        &self.receipts
    }

    /// Admit the complete receipt of one positioned runtime enactment of this block.
    ///
    /// # Errors
    /// [`RuntimeEnactmentResultsErrorV1`] for the first rule the receipt breaks; the custody
    /// is unchanged.
    pub fn admit(
        &mut self,
        receipt: RuntimeEnactmentReceiptV1,
    ) -> Result<(), RuntimeEnactmentResultsErrorV1> {
        use RuntimeEnactmentResultsErrorV1 as Error;
        receipt.validate().map_err(Error::Receipt)?;
        if receipt.anchor.height != self.height || receipt.anchor.timestamp_ms != self.timestamp_ms
        {
            return Err(Error::OtherBlock);
        }
        if self
            .receipts
            .last()
            .is_some_and(|last| receipt.anchor.position <= last.anchor.position)
        {
            return Err(Error::Position);
        }
        if self
            .receipts
            .iter()
            .any(|held| held.governance_attempt_id == receipt.governance_attempt_id)
        {
            return Err(Error::DuplicateAttempt);
        }
        let bytes = norito::canonical_frame_len(&receipt).map_err(|_| Error::Oversized)?;
        if !u64::try_from(bytes).is_ok_and(|bytes| bytes <= RUNTIME_ENACTMENT_RECEIPT_MAX_BYTES_V1)
        {
            return Err(Error::Oversized);
        }
        if !self.fits() {
            return Err(Error::Capacity);
        }
        self.receipts
            .try_reserve(1)
            .map_err(|_| Error::Allocation)?;
        self.receipts.push(receipt);
        Ok(())
    }

    /// Consume the custody into its receipts, in pass order.
    #[must_use]
    pub fn finish(self) -> Vec<RuntimeEnactmentReceiptV1> {
        self.receipts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(status: GovernanceAttemptStatusV1) -> RuntimeEnactmentReceiptV1 {
        RuntimeEnactmentReceiptV1 {
            anchor: GovernanceAnchorV1 {
                height: 19,
                position: 2,
                timestamp_ms: 300_000,
            },
            governance_attempt_id: GovernanceAttemptId::new([0x42; 32]),
            status,
            stage: GovernanceStageV1::default(),
            attempt_state_digest: [0x53; 64].into(),
        }
    }

    /// A terminal receipt of attempt `tag` at `position` of block 19.
    fn positioned(tag: u8, position: u32) -> RuntimeEnactmentReceiptV1 {
        RuntimeEnactmentReceiptV1 {
            anchor: GovernanceAnchorV1 {
                height: 19,
                position,
                timestamp_ms: 300_000,
            },
            governance_attempt_id: GovernanceAttemptId::new([tag; 32]),
            ..receipt(GovernanceAttemptStatusV1::Enacted)
        }
    }

    #[test]
    fn standalone_runtime_receipt_roundtrips_each_original_terminal_outcome() {
        for status in [
            GovernanceAttemptStatusV1::Enacted,
            GovernanceAttemptStatusV1::Superseded,
            GovernanceAttemptStatusV1::ExecutionFailed,
        ] {
            let original = receipt(status);
            original.validate().unwrap();
            let bytes = norito::encode_canonical(&original).unwrap();
            assert_eq!(
                norito::decode_canonical::<RuntimeEnactmentReceiptV1>(&bytes).unwrap(),
                original
            );
            let json = norito::json::to_json(&original).unwrap();
            assert_eq!(
                norito::json::from_str::<RuntimeEnactmentReceiptV1>(&json).unwrap(),
                original
            );
        }
    }

    #[test]
    fn standalone_runtime_receipt_rejects_a_missing_position_or_nonterminal_outcome() {
        let mut invalid = receipt(GovernanceAttemptStatusV1::Certified);
        assert_eq!(
            invalid.validate(),
            Err(RuntimeEnactmentReceiptErrorV1::NonterminalOutcome)
        );
        invalid.anchor.height = 0;
        assert_eq!(
            invalid.validate(),
            Err(RuntimeEnactmentReceiptErrorV1::ZeroHeight)
        );
    }

    #[test]
    fn standalone_runtime_receipt_is_not_reachable_from_the_native_block_graph() {
        let native = crate::block::SignedBlock::schema();
        let own = RuntimeEnactmentReceiptV1::schema();
        let receipt_id = core::any::TypeId::of::<RuntimeEnactmentReceiptV1>();
        assert!(own.iter().any(|(id, _)| *id == receipt_id));
        assert!(native.iter().all(|(id, _)| *id != receipt_id));
        assert!(
            native
                .iter()
                .all(|(_, entry)| !entry.type_id.contains("RuntimeEnactmentReceiptV1"))
        );
        assert!(
            native
                .iter()
                .all(|(_, entry)| !entry.type_id.contains("RuntimeEnactmentResults"))
        );
        assert!(crate::wire_schema::wire_root_defects(&native).is_empty());
    }

    #[test]
    fn result_closure_names_the_exact_bounds_of_this_custody() {
        let closure = core::str::from_utf8(RUNTIME_ENACTMENT_RESULT_SERVICE_CLOSURE_V1).unwrap();
        assert!(closure.starts_with("iroha.governance.parliament.anonymous.v1:"));
        assert!(closure.contains(&format!(
            ":per-block-{RUNTIME_ENACTMENT_RESULTS_PER_BLOCK_V1}:"
        )));
        assert!(closure.contains(&format!(
            ":receipt-max-{RUNTIME_ENACTMENT_RECEIPT_MAX_BYTES_V1}:"
        )));
        assert!(closure.ends_with(":admit-before-apply-v1"));
    }

    #[test]
    fn every_terminal_receipt_frame_fits_its_cap() {
        for status in [
            GovernanceAttemptStatusV1::Enacted,
            GovernanceAttemptStatusV1::Superseded,
            GovernanceAttemptStatusV1::ExecutionFailed,
        ] {
            let mut widest = receipt(status);
            widest.anchor = GovernanceAnchorV1 {
                height: u64::MAX,
                position: u32::MAX,
                timestamp_ms: u64::MAX,
            };
            widest.attempt_state_digest = [0xff; 64].into();
            let bytes = norito::canonical_frame_len(&widest).unwrap();
            eprintln!("runtime enactment receipt frame ({status:?}): {bytes} bytes");
            assert!(u64::try_from(bytes).unwrap() <= RUNTIME_ENACTMENT_RECEIPT_MAX_BYTES_V1);
        }
    }

    #[test]
    fn result_custody_admits_its_bound_in_pass_order_then_refuses_unchanged() {
        let mut results = RuntimeEnactmentResultsV1::new(19, 300_000);
        assert!(results.is_empty());
        let bound = RUNTIME_ENACTMENT_RESULTS_PER_BLOCK_V1;
        for index in 0..bound {
            assert!(results.fits());
            let tag = u8::try_from(index).unwrap();
            results.admit(positioned(tag, 2 * index + 1)).unwrap();
        }
        assert!(!results.fits());
        assert_eq!(results.receipts().len(), usize::try_from(bound).unwrap());
        let held = results.receipts().to_vec();
        assert_eq!(
            results.admit(positioned(0xee, 4 * bound)),
            Err(RuntimeEnactmentResultsErrorV1::Capacity)
        );
        assert_eq!(results.receipts(), held.as_slice());
        assert_eq!(results.finish(), held);
    }

    #[test]
    fn result_custody_refuses_foreign_reordered_repeated_or_invalid_receipts() {
        use RuntimeEnactmentResultsErrorV1 as Error;
        let mut results = RuntimeEnactmentResultsV1::new(19, 300_000);
        results.admit(positioned(1, 4)).unwrap();
        let held = results.receipts().to_vec();

        let mut other_height = positioned(2, 5);
        other_height.anchor.height = 20;
        let mut other_time = positioned(2, 5);
        other_time.anchor.timestamp_ms = 300_001;
        let mut nonterminal = positioned(2, 5);
        nonterminal.status = GovernanceAttemptStatusV1::Certified;
        let mut zero = positioned(2, 5);
        zero.anchor.height = 0;
        for (candidate, expected) in [
            (other_height, Error::OtherBlock),
            (other_time, Error::OtherBlock),
            (positioned(2, 4), Error::Position),
            (positioned(2, 3), Error::Position),
            (positioned(1, 9), Error::DuplicateAttempt),
            (
                nonterminal,
                Error::Receipt(RuntimeEnactmentReceiptErrorV1::NonterminalOutcome),
            ),
            (
                zero,
                Error::Receipt(RuntimeEnactmentReceiptErrorV1::ZeroHeight),
            ),
        ] {
            assert_eq!(results.admit(candidate), Err(expected));
            assert_eq!(results.receipts(), held.as_slice());
        }
        assert_eq!((results.height(), results.timestamp_ms()), (19, 300_000));
        results.admit(positioned(2, 5)).unwrap();
        assert_eq!(results.receipts().len(), 2);
    }
}

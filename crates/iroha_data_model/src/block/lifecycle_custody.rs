//! Non-wire original allocation custody for locally produced lifecycle rows.

use iroha_allocation::{AllocationCharge, ChargedBuffer};

/// Charges that follow the exact locally produced output graph into its block.
/// Decoding and cloning create independent graphs requiring their own admission.
/// The local custody is deliberately absent from codecs, schema and comparison.
#[derive(Default)]
pub struct LifecycleOutputCustody {
    ledgers: Option<ChargedBuffer<ChargedBuffer<AllocationCharge>>>,
}

impl Clone for LifecycleOutputCustody {
    fn clone(&self) -> Self {
        // A graph clone never inherits the original allocations' admission.
        Self::default()
    }
}

impl core::fmt::Debug for LifecycleOutputCustody {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LifecycleOutputCustody")
            .field(
                "rows",
                &self
                    .ledgers
                    .as_ref()
                    .map_or(0, |rows| rows.as_slice().len()),
            )
            .finish()
    }
}

impl LifecycleOutputCustody {
    /// Retain the original per-row ledgers until their canonical allocations die.
    ///
    /// # Safety
    /// Each charge must accompany its exact allocation in the output rows. The
    /// caller must keep rows ahead of custody in destruction order, including
    /// refusal/unwind, and move both to the same immutable block before escape.
    #[allow(unsafe_code)]
    pub unsafe fn new(ledgers: ChargedBuffer<ChargedBuffer<AllocationCharge>>) -> Self {
        Self {
            ledgers: Some(ledgers),
        }
    }
}

impl super::SignedBlock {
    /// Attach original charge custody immediately after moving its exact rows.
    ///
    /// # Safety
    /// `custody` must belong to the exact graph just installed by
    /// `set_execution_outputs`. No fallible work or exposure may separate those
    /// two moves. This function allocates nothing and cannot replace custody.
    #[cfg(any(test, feature = "transparent_api"))]
    #[allow(unsafe_code)]
    pub unsafe fn retain_lifecycle_output_custody(&mut self, custody: LifecycleOutputCustody) {
        let result = self
            .result
            .as_mut()
            .expect("just attached original output graph");
        assert!(result.lifecycle_custody.ledgers.is_none());
        result.lifecycle_custody = custody;
    }
}

#[cfg(all(test, feature = "transparent_api"))]
mod tests {
    use super::*;
    use crate::block::{execution_output::*, output_test_support as fixture};
    use iroha_allocation::AllocationBudget;

    #[test]
    #[allow(unsafe_code)]
    fn lifecycle_custody_follows_original_block_without_changing_codec_or_cloning_admission() {
        let pool = AllocationBudget::new(1_000_000);
        let mut ledgers = ChargedBuffer::new(1, &pool).unwrap();
        let mut ledger = ChargedBuffer::new(1, &pool).unwrap();
        let effects = ChargedBuffer::<TriggerLifecycleEffectV1>::new(1, &pool).unwrap();
        // SAFETY: this spare-capacity original Vec moves into the exact row.
        let (effects, charge) = unsafe { effects.into_allocation_parts() };
        let pointer = effects.as_ptr();
        ledger.push_reserved(charge);
        ledgers.push_reserved(ledger);
        let mut block = fixture::proposal(1);
        let ExecutionOutputV1::Network(execution) = fixture::network(0, Ok(Vec::new())) else {
            unreachable!()
        };
        fixture::install(
            &mut block,
            vec![ExecutionOutputV1::NetworkWithLifecycle(
                NetworkExecutionOutputV2 {
                    execution,
                    trigger_lifecycle: TriggerLifecycleEffectsV1::Complete { effects },
                },
            )],
            0,
        )
        .unwrap();
        // SAFETY: all ledger allocations remain owned by the exact row just moved.
        unsafe {
            block.retain_lifecycle_output_custody(LifecycleOutputCustody::new(ledgers));
        }
        assert_eq!(
            block.execution_outputs()[0]
                .require_trigger_lifecycle()
                .unwrap()
                .effects()
                .as_ptr(),
            pointer
        );
        let reserved = pool.reserved_bytes();
        assert!(reserved > 0);
        let cloned = block.clone();
        let bytes = block.encode_wire().unwrap();
        assert_eq!(bytes, cloned.encode_wire().unwrap());
        assert_eq!(pool.reserved_bytes(), reserved);
        let decoded = crate::block::decode_framed_signed_block(&bytes).unwrap();
        assert_eq!(decoded, block);
        drop(block);
        assert_eq!(pool.reserved_bytes(), 0);
        assert_eq!(cloned.encode_wire().unwrap(), bytes);
    }
}

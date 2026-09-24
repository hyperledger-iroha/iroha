//! Pre-effect custody for ordinary canonical carrier membership source hashes.

use super::*;
use mv::allocation::{AllocationBudget, AllocationRefusal, ChargedBuffer, ChargedBufferError};

/// The exact signed source and its fixed, originally funded membership hashes.
///
/// A source is reserved before acquiring the block owner, populated against that
/// owner's pre-block World, and retained until the metadata tail stages the tip.
pub(super) struct PrepaidOrdinaryCarrierMembership {
    source_header: HashOf<BlockHeader>,
    hashes: ChargedBuffer<HashOf<TransactionEntrypoint>>,
}

impl PrepaidOrdinaryCarrierMembership {
    fn maximum_hash_count(entrypoints: usize) -> Result<usize, MembershipAdmissionError> {
        entrypoints
            .checked_mul(2)
            .ok_or_else(|| MembershipAdmissionError::Capacity(AllocationRefusal::DemandOverflow))
    }

    pub(super) fn reserve(
        source: &SignedBlock,
        budget: &AllocationBudget,
    ) -> Result<Self, MembershipAdmissionError> {
        Self::reserve_with(source, budget, ChargedBuffer::new)
    }

    fn reserve_with(
        source: &SignedBlock,
        budget: &AllocationBudget,
        allocate: impl FnOnce(
            usize,
            &AllocationBudget,
        ) -> Result<
            ChargedBuffer<HashOf<TransactionEntrypoint>>,
            ChargedBufferError,
        >,
    ) -> Result<Self, MembershipAdmissionError> {
        let maximum = Self::maximum_hash_count(source.external_entrypoint_count())?;
        Ok(Self {
            source_header: source.hash(),
            hashes: allocate(maximum, budget)?,
        })
    }

    /// Authenticate sealed-reveal aliases from this original pre-block scope.
    pub(super) fn fill_from_preblock(&mut self, block: &StateBlock<'_>, source: &SignedBlock) {
        assert_eq!(
            self.source_header,
            source.hash(),
            "reserved carrier changed"
        );
        self.hashes.truncate(0);
        crate::tx::for_each_canonical_carrier_membership_hash(
            block,
            source.external_entrypoints_slice(),
            |hash| {
                self.hashes
                    .append(std::slice::from_ref(&hash))
                    .expect("two funded identities per canonical entrypoint");
            },
        );
    }

    /// Check the output tail still names the exact funded signed source.
    pub(super) fn matches(&self, block: &StateBlock<'_>, source: &SignedBlock) -> bool {
        if self.source_header != source.hash() {
            return false;
        }
        let mut index = 0;
        let mut matches = true;
        crate::tx::for_each_canonical_carrier_membership_hash(
            block,
            source.external_entrypoints_slice(),
            |hash| {
                matches &= self.hashes.as_slice().get(index) == Some(&hash);
                index += 1;
            },
        );
        matches && index == self.hashes.as_slice().len()
    }

    pub(super) fn hashes_mut(&mut self) -> &mut [HashOf<TransactionEntrypoint>] {
        self.hashes.as_mut_slice()
    }

    #[cfg(test)]
    pub(super) fn capacity(&self) -> usize {
        self.hashes.capacity()
    }
}

impl StateBlock<'_> {
    /// Stage the exact signed ordinary source from its original funded backing.
    ///
    /// This is shared by the output seal and the later carrier metadata tail;
    /// repeat staging checks the same source and reuses the already charged tip.
    pub(crate) fn stage_prepaid_ordinary_carrier_membership(
        &mut self,
        signed_block: &SignedBlock,
        height: NonZeroUsize,
    ) -> Result<(), MergeLedgerCommitError> {
        if let Some(mut prepaid) = self.ordinary_carrier_membership_source.take() {
            if !prepaid.matches(self, signed_block) {
                self.ordinary_carrier_membership_source = Some(prepaid);
                return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                    "ordinary membership source differs from its signed carrier".to_owned(),
                ));
            }
            let result =
                self.stage_canonical_carrier_membership_from_slice(prepaid.hashes_mut(), height);
            // Idempotent tip staging sorts its borrowed slice. Restore the
            // signed order in the same fixed allocation before another tail.
            prepaid.fill_from_preblock(self, signed_block);
            self.ordinary_carrier_membership_source = Some(prepaid);
            result
        } else if signed_block.external_entrypoints_slice().is_empty() {
            // Native and certified-merge carriers have no ordinary source.
            self.stage_canonical_carrier_membership_from_slice(&mut [], height)
        } else {
            Err(MergeLedgerCommitError::MembershipAdmission(
                storage_transactions::MembershipAdmissionError::SourceNotFunded,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{
        block::builder::BlockBuilder,
        prelude::{Log, TransactionBuilder},
        transaction::FeePaymentIntent,
    };
    use nonzero_ext::nonzero;

    fn signed_source() -> SignedBlock {
        let transaction = TransactionBuilder::new(
            iroha_data_model::NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"source test network")),
            ),
            iroha_test_samples::ALICE_ID.clone(),
            FeePaymentIntent::authority(vec![], None),
        )
        .with_instructions([Log::new(
            iroha_data_model::prelude::Level::INFO,
            "allocator refusal".to_owned(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
        let mut builder = BlockBuilder::new(BlockHeader::new(nonzero!(1_u64), None, None, 1, 0));
        builder.push_transaction(transaction);
        builder.build_with_signature(0, iroha_test_samples::ALICE_KEYPAIR.private_key())
    }

    #[test]
    fn maximum_alias_count_is_checked_before_any_allocation() {
        assert_eq!(
            PrepaidOrdinaryCarrierMembership::maximum_hash_count(1).unwrap(),
            2
        );
        assert!(matches!(
            PrepaidOrdinaryCarrierMembership::maximum_hash_count(usize::MAX),
            Err(MembershipAdmissionError::Capacity(
                AllocationRefusal::DemandOverflow
            ))
        ));
    }

    #[test]
    fn allocator_refusal_preserves_unspent_original_pool_and_source() {
        let source = signed_source();
        let original_hash = source.hash();
        let budget = AllocationBudget::new(1024);
        let requested_bytes = std::alloc::Layout::array::<HashOf<TransactionEntrypoint>>(2)
            .unwrap()
            .size();
        let error =
            PrepaidOrdinaryCarrierMembership::reserve_with(&source, &budget, |maximum, _| {
                assert_eq!(maximum, 2);
                Err(ChargedBufferError::Allocator { requested_bytes })
            })
            .err()
            .expect("injected allocator refusal must be local");
        assert!(matches!(
            error,
            MembershipAdmissionError::Allocator {
                requested_bytes: actual
            } if actual == requested_bytes
        ));
        assert_eq!(budget.reserved_bytes(), 0);
        assert_eq!(source.hash(), original_hash);
        let admitted = PrepaidOrdinaryCarrierMembership::reserve(&source, &budget)
            .expect("same signed source retries with the unchanged pool");
        assert_eq!(admitted.capacity(), 2);
        drop(admitted);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

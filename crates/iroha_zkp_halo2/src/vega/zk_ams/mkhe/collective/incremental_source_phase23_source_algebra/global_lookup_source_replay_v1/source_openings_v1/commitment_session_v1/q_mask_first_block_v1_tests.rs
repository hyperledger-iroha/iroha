//! Actual Production entropy branch under isolated, explicitly unqualified axes.
use super::*;
use crate::testing::TestDirectory;
use crate::vega::zk_ams::mkhe::global_lookup_statement_v1::{
    OrderedPlaneSpoolSnapshotV1, OrderedPlaneSpoolWriterV1, OrderedStorageSessionBudgetV1,
};
use std::{cell::Cell, rc::Rc};

struct Words {
    next: Rc<Cell<u64>>,
    calls: Rc<Cell<usize>>,
    drops: Rc<Cell<usize>>,
    constant: Option<u64>,
    fail: Option<(usize, bool)>,
}
impl Words {
    fn new() -> Self {
        Self {
            next: Rc::new(Cell::new(0)),
            calls: Rc::new(Cell::new(0)),
            drops: Rc::new(Cell::new(0)),
            constant: None,
            fail: None,
        }
    }
}
impl Drop for Words {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
impl MaskedRelaxedRandomSourceV1 for Words {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        if let Some((at, panic)) = self.fail {
            if call == at {
                destination[..3].fill(77);
                if panic {
                    panic!("original source partially filled then unwound");
                }
                return Err(MaskedRelaxedRandomErrorV1::Unavailable);
            }
        }
        for bytes in destination.chunks_mut(8) {
            let value = self.constant.unwrap_or(self.next.get());
            bytes.copy_from_slice(&value.to_le_bytes()[..bytes.len()]);
            self.next.set(self.next.get() + 1);
        }
        Ok(())
    }
}
fn session(random: Words) -> GlobalLookupCommitmentSessionLiveV1<Words> {
    // This is solely a sampler fixture. It supplies no replay evidence,
    // admitted native40 source, original earlier commitment proofs or seal.
    GlobalLookupCommitmentSessionLiveV1 {
        proof_resources: Default::default(),
        entropy: GlobalLookupProofSessionEntropySourceV1::Production {
            original_random: random,
            commitment_entropy_bytes: 0,
            q_mask_entropy_bytes: 0,
        },
        inventory: GlobalLookupCommitmentInventorySkeletonV1::new_v1().unwrap(),
        proof_session_context_digest: [11; 32],
        source_opening_context_digest: Some([12; 32]),
        next_global_ordinal: 27_176,
        next_purpose: GlobalLookupCommitmentPurposeV1::QMaskDigit,
        next_purpose_ordinal: 0,
        pending_source: None,
    }
}
fn pair(
    directory: &std::path::Path,
) -> (OrderedPlaneSpoolSnapshotV1, OrderedStorageSessionBudgetV1) {
    let mut budget = OrderedStorageSessionBudgetV1::new_v1();
    let mut writer =
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(directory, [81; 32], &mut budget)
            .unwrap();
    for slot in 0..66 {
        let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
        if slot % 33 == 32 {
            chunk.as_mut_slice_v1()[31] = 1;
            chunk.as_mut_slice_v1()[32..65].copy_from_slice(
                &Point::canonical_generator()
                    .unwrap()
                    .to_non_identity_wire_bytes()
                    .unwrap(),
            );
        }
        writer.write_slot_v1(slot, chunk).unwrap();
    }
    (writer.seal_v1().unwrap(), budget)
}
#[test]
#[cfg(unix)]
fn qmask_first_block_continues_original_entropy_and_retains_exact_preimage() {
    let directory = TestDirectory::new("qmask-first-sample");
    let (pair, storage) = pair(directory.path());
    let plan = pair.q_mask_s_file_plan_v1().unwrap();
    let random = Words::new();
    let calls = Rc::clone(&random.calls);
    let drops = Rc::clone(&random.drops);
    let mut original = session(random);
    let mut earlier = Zeroizing::new([0; 32]);
    fill_entropy_v1(&mut original.entropy, 1, 0, &mut earlier[..]).unwrap();
    let (memory, file_memory) = QMaskFirstBlockMemoryV1::new_v1(&mut original, &plan).unwrap();
    let mut file = pair
        .create_q_mask_s_file_v1(directory.path(), plan, file_memory)
        .unwrap();
    let block = SampledQMaskSBlockV1::sample_v1(&mut original, memory).unwrap();
    assert_eq!(calls.get(), 1 + BLOCK_COEFFICIENTS_V1);
    assert_eq!(block.coefficients.values.len(), BLOCK_COEFFICIENTS_V1);
    for (index, value) in block.coefficients.values.iter().enumerate() {
        assert_eq!(*value, index as u64 + 4);
    }
    let GlobalLookupProofSessionEntropySourceV1::Production {
        commitment_entropy_bytes,
        q_mask_entropy_bytes,
        ..
    } = &original.entropy
    else {
        unreachable!()
    };
    assert_eq!(
        (*commitment_entropy_bytes, *q_mask_entropy_bytes),
        (32, 131_072)
    );
    assert_eq!(original.next_global_ordinal, 27_176);
    assert!(original.inventory.slots[27_176].is_none());
    block.write_slots_v1(&mut file).unwrap();
    let closed = file.finish_block_v1().unwrap();
    assert_eq!(
        storage.test_usage_words_v1()[3],
        2 * 66 * 16_400 + 8 * 16_400
    );
    let (retry_memory, unused_file_memory) =
        QMaskFirstBlockMemoryV1::new_v1(&mut original, &pair.q_mask_s_file_plan_v1().unwrap())
            .unwrap();
    assert!(matches!(
        SampledQMaskSBlockV1::sample_v1(&mut original, retry_memory),
        Err(QMaskSErrorV1::Source)
    ));
    drop(unused_file_memory);
    assert_eq!(calls.get(), 1 + BLOCK_COEFFICIENTS_V1);
    drop(closed);
    drop(block);
    assert_eq!(original.proof_resources.live_bytes().unwrap(), 0);
    drop(original);
    assert_eq!(drops.get(), 1);
}
#[test]
fn qmask_uniform_sampler_uses_exact_acceptance_zone_and_bounded_original_draws() {
    for modulus in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1 {
        let zone = u64::MAX - u64::MAX % modulus;
        assert_eq!(zone / modulus, 16);
        assert_eq!(zone % modulus, 0);
    }
    let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0];
    let mut original = Words::new();
    original.constant = Some(u64::MAX);
    let calls = Rc::clone(&original.calls);
    let mut used = 0;
    let mut borrow = MaskEntropyBorrowV1 {
        random: &mut original,
        attempted_bytes: &mut used,
    };
    assert_eq!(
        sample_below(modulus, &mut borrow),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    );
    assert_eq!(calls.get(), 128);
    assert_eq!(used, 1024);
    original.constant = Some(0);
    let mut used = 0;
    let mut borrow = MaskEntropyBorrowV1 {
        random: &mut original,
        attempted_bytes: &mut used,
    };
    assert_eq!(sample_below(modulus, &mut borrow), Ok(0));
    assert_eq!(used, 8);
    original.constant = Some(modulus - 1);
    let mut used = 0;
    let mut borrow = MaskEntropyBorrowV1 {
        random: &mut original,
        attempted_bytes: &mut used,
    };
    assert_eq!(sample_below(modulus, &mut borrow), Ok(modulus - 1));
    assert_eq!(used, 8);
    struct BoundaryWords {
        calls: usize,
        rejected: usize,
        zone: u64,
    }
    impl MaskedRelaxedRandomSourceV1 for BoundaryWords {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            let value = if self.calls < self.rejected {
                self.zone
            } else {
                self.zone - 1
            };
            self.calls += 1;
            destination.copy_from_slice(&value.to_le_bytes());
            Ok(())
        }
    }
    for rejected in [0, 1, 127, 128] {
        let mut random = BoundaryWords {
            calls: 0,
            rejected,
            zone: u64::MAX - u64::MAX % modulus,
        };
        let mut used = 0;
        let result = sample_below(
            modulus,
            &mut MaskEntropyBorrowV1 {
                random: &mut random,
                attempted_bytes: &mut used,
            },
        );
        if rejected < 128 {
            assert_eq!(result, Ok(modulus - 1));
        } else {
            assert_eq!(result, Err(ZkAmsMkheErrorV1::RandomUnavailable));
        }
        assert_eq!(random.calls, (rejected + 1).min(128));
        assert_eq!(used, random.calls as u64 * 8);
    }
}
#[test]
fn qmask_entropy_ceiling_and_partial_failures_charge_before_read_without_refund() {
    let mut original = Words::new();
    let calls = Rc::clone(&original.calls);
    let mut used = MASK_ENTROPY_MAX_BYTES_V1 - 8;
    let mut destination = [55; 8];
    {
        let mut borrow = MaskEntropyBorrowV1 {
            random: &mut original,
            attempted_bytes: &mut used,
        };
        borrow.fill_bytes(&mut destination).unwrap();
    }
    assert_eq!(used, MASK_ENTROPY_MAX_BYTES_V1);
    assert_eq!(calls.get(), 1);
    {
        let mut borrow = MaskEntropyBorrowV1 {
            random: &mut original,
            attempted_bytes: &mut used,
        };
        assert!(borrow.fill_bytes(&mut destination).is_err());
    }
    assert_eq!(destination, [0; 8]);
    assert_eq!(calls.get(), 1);
    for initial in [0, u64::MAX] {
        let mut used = initial;
        let mut short = [55; 7];
        let mut borrow = MaskEntropyBorrowV1 {
            random: &mut original,
            attempted_bytes: &mut used,
        };
        assert!(borrow.fill_bytes(&mut short).is_err());
        assert_eq!(short, [0; 7]);
        assert_eq!(used, initial);
    }
    let mut used = 0;
    original.fail = Some((1, false));
    destination.fill(55);
    {
        let mut borrow = MaskEntropyBorrowV1 {
            random: &mut original,
            attempted_bytes: &mut used,
        };
        assert!(borrow.fill_bytes(&mut destination).is_err());
    }
    assert_eq!(used, 8);
    assert_eq!(destination, [0; 8]);
    original.fail = Some((2, true));
    let mut used = 0;
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut borrow = MaskEntropyBorrowV1 {
            random: &mut original,
            attempted_bytes: &mut used,
        };
        let _ = sample_below(ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0], &mut borrow);
    }));
    assert!(panic.is_err());
    assert_eq!(used, 8);
}
#[test]
fn qmask_uniform_block_top_zero_and_invalid_geometry_preserve_exact_draw_counts() {
    let mut original = Words::new();
    original.constant = Some(0);
    let calls = Rc::clone(&original.calls);
    assert!(matches!(
        sample_block_v1(&mut original, 40, 0),
        Err(QMaskSErrorV1::Source)
    ));
    assert!(matches!(
        sample_block_v1(&mut original, 0, 8),
        Err(QMaskSErrorV1::Source)
    ));
    assert_eq!(calls.get(), 0);
    let zero = sample_block_v1(&mut original, 0, 0).unwrap();
    assert!(zero.values.iter().all(|v| *v == 0));
    assert_eq!(calls.get(), 16_384);
    drop(zero);
    let top = sample_block_v1(&mut original, 39, 7).unwrap();
    assert!(top.values.iter().all(|v| *v == 0));
    assert_eq!(top.values[16_383], 0);
    assert_eq!(calls.get(), 32_767);
}
#[test]
fn qmask_failed_or_unwound_sampling_clears_every_initialized_prefix() {
    for panic in [false, true] {
        let mut original = Words::new();
        original.fail = Some((9, panic));
        let calls = Rc::clone(&original.calls);
        let drops = Rc::clone(&original.drops);
        let before = ZEROIZED_MASK_VALUES_V1.with(Cell::get);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let result = sample_block_v1(&mut original, 0, 0);
            assert!(matches!(result, Err(QMaskSErrorV1::Entropy)));
        }));
        assert_eq!(result.is_err(), panic);
        assert_eq!(calls.get(), 10);
        assert_eq!(drops.get(), 1);
        assert_eq!(ZEROIZED_MASK_VALUES_V1.with(Cell::get) - before, 9);
    }
}
#[test]
#[cfg(unix)]
fn qmask_first_memory_capacity_and_foreign_ledger_reject_before_entropy() {
    let directory = TestDirectory::new("qmask-memory");
    let (pair, _storage) = pair(directory.path());
    let plan = pair.q_mask_s_file_plan_v1().unwrap();
    let random = Words::new();
    let calls = Rc::clone(&random.calls);
    let mut original = session(random);
    let (memory, file_memory) = QMaskFirstBlockMemoryV1::new_v1(&mut original, &plan).unwrap();
    let exact = original.proof_resources.live_bytes().unwrap();
    drop(memory);
    drop(file_memory);
    original.proof_resources.set_test_workspace_limit_v1(exact);
    let competing = original.proof_resources.reserve_workspace_v1(1, 0).unwrap();
    assert!(matches!(
        QMaskFirstBlockMemoryV1::new_v1(&mut original, &plan),
        Err(QMaskSErrorV1::Capacity)
    ));
    assert_eq!(original.proof_resources.live_bytes().unwrap(), 1);
    assert_eq!(calls.get(), 0);
    drop(competing);
    let (memory, file_memory) = QMaskFirstBlockMemoryV1::new_v1(&mut original, &plan).unwrap();
    assert_eq!(original.proof_resources.live_bytes().unwrap(), exact);
    let other_random = Words::new();
    let other_calls = Rc::clone(&other_random.calls);
    let mut other = session(other_random);
    assert!(matches!(
        SampledQMaskSBlockV1::sample_v1(&mut other, memory),
        Err(QMaskSErrorV1::Source)
    ));
    assert_eq!(other_calls.get(), 0);
    assert_eq!(calls.get(), 0);
    drop(file_memory);
    let (memory, file_memory) = QMaskFirstBlockMemoryV1::new_v1(&mut original, &plan).unwrap();
    let block = SampledQMaskSBlockV1::sample_v1(&mut original, memory).unwrap();
    assert_eq!(calls.get(), 16_384);
    drop(file_memory);
    let block_charge = original.proof_resources.live_bytes().unwrap();
    assert!(block_charge >= 131_072);
    // The coefficient result owns its reservation even after the file budget is
    // gone; dropping the actual block releases the final live credit.
    drop(block);
    assert_eq!(original.proof_resources.live_bytes().unwrap(), 0);
}
#[test]
#[cfg(unix)]
fn qmask_first_coordinate_and_file_binding_reject_without_alternate_openings() {
    let directory = TestDirectory::new("qmask-binding");
    let (pair, storage) = pair(directory.path());
    let plan = pair.q_mask_s_file_plan_v1().unwrap();
    let random = Words::new();
    let calls = Rc::clone(&random.calls);
    let mut original = session(random);
    for ordinal in [0, 27_175, 27_177] {
        original.next_global_ordinal = ordinal;
        assert!(matches!(
            QMaskFirstBlockMemoryV1::new_v1(&mut original, &plan),
            Err(QMaskSErrorV1::Source)
        ));
    }
    assert_eq!(calls.get(), 0);
    original.next_global_ordinal = 27_176;
    let (memory, file_memory) = QMaskFirstBlockMemoryV1::new_v1(&mut original, &plan).unwrap();
    let block = SampledQMaskSBlockV1::sample_v1(&mut original, memory).unwrap();
    let (other, _other_storage) = self::pair(directory.path());
    let other_plan = other.q_mask_s_file_plan_v1().unwrap();
    let mut unrelated_resources=crate::vega::zk_ams::mkhe::rns_native_resource_budget::RnsNativeProofResourceBudgetV1::default();
    let other_memory = other_plan
        .reserve_memory_v1(&mut unrelated_resources)
        .unwrap();
    let mut wrong = other
        .create_q_mask_s_file_v1(directory.path(), other_plan, other_memory)
        .unwrap();
    drop(file_memory);
    let before = storage.test_usage_words_v1();
    assert_eq!(block.write_slots_v1(&mut wrong), Err(QMaskSErrorV1::Source));
    assert_eq!(storage.test_usage_words_v1(), before);
    assert!(
        original.inventory.slots[27_176..]
            .iter()
            .all(Option::is_none)
    );
}

#[test]
#[cfg(unix)]
fn qmask_first_failed_entropy_erases_prefix_and_prevents_original_rng_reentry() {
    let directory = TestDirectory::new("qmask-failed-first");
    let (pair, _storage) = pair(directory.path());
    let plan = pair.q_mask_s_file_plan_v1().unwrap();
    for panic in [false, true] {
        let mut random = Words::new();
        random.fail = Some((9, panic));
        let calls = Rc::clone(&random.calls);
        let drops = Rc::clone(&random.drops);
        let mut original = session(random);
        let (memory, file_memory) = QMaskFirstBlockMemoryV1::new_v1(&mut original, &plan).unwrap();
        let before = ZEROIZED_MASK_VALUES_V1.with(Cell::get);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            SampledQMaskSBlockV1::sample_v1(&mut original, memory)
        }));
        assert_eq!(result.is_err(), panic);
        assert!(matches!(result, Err(_) | Ok(Err(QMaskSErrorV1::Entropy))));
        assert_eq!(calls.get(), 10);
        assert_eq!(ZEROIZED_MASK_VALUES_V1.with(Cell::get) - before, 9);
        let GlobalLookupProofSessionEntropySourceV1::Production {
            q_mask_entropy_bytes,
            commitment_entropy_bytes,
            ..
        } = &original.entropy
        else {
            unreachable!()
        };
        assert_eq!((*q_mask_entropy_bytes, *commitment_entropy_bytes), (80, 0));
        // This lower fixture still holds the failed session for this hostile
        // retry; the real retained transition already takes/destroys its phase.
        let (retry, extra_file_memory) =
            QMaskFirstBlockMemoryV1::new_v1(&mut original, &plan).unwrap();
        assert!(matches!(
            SampledQMaskSBlockV1::sample_v1(&mut original, retry),
            Err(QMaskSErrorV1::Source)
        ));
        assert_eq!(calls.get(), 10);
        drop((file_memory, extra_file_memory));
        assert_eq!(original.proof_resources.live_bytes().unwrap(), 0);
        drop(original);
        assert_eq!(drops.get(), 1);
    }
}

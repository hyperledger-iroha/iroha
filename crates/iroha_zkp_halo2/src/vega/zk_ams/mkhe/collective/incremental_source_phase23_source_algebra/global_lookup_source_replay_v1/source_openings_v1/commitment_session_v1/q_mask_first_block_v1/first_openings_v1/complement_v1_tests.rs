//! Exact complement controls with an explicitly synthetic preceding S inventory.
//! Actual files, original ledgers/entropy and tested complement MSMs are real;
//! synthetic earlier tickets do not qualify an authenticated native40 source.
use super::super::tests::{self as previous, Fault, Fixture, Random};
use super::*;
use crate::{
    generalized_bulletproof::{
        ProofSuite, SecretMultiexpBuilder, secret_u15_msm_v1::test_controls_v1 as controls,
    },
    testing::TestDirectory,
    vega::{
        bulletproof_t256::{
            ZkAmsT256BulletproofSuiteV1 as Suite, zeroizing_t256_scalar_vec_drop_count_v1,
        },
        zk_ams::mkhe::{
            global_lookup_statement_v1::{
                OrderedPlaneSpoolWriterV1, OrderedStorageSessionBudgetV1,
            },
            rns_native_profile::ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1,
        },
    },
};
use std::cell::Cell;

#[derive(Clone, Copy)]
enum Malformed {
    None,
    Residue,
    Top,
}
struct Completed {
    session: GlobalLookupCommitmentSessionLiveV1<Random>,
    table: RnsNativeU15MsmTableV1,
    source: CompleteQMaskSOpeningsV1,
    file: SealedQMaskSFileV1,
    storage: OrderedStorageSessionBudgetV1,
}
impl Completed {
    fn random(&self) -> &Random {
        match &self.session.entropy {
            GlobalLookupProofSessionEntropySourceV1::Production {
                original_random, ..
            } => original_random,
            _ => unreachable!(),
        }
    }
    fn begin(&mut self) -> Result<QMaskComplementOpeningsV1, QMaskSErrorV1> {
        QMaskComplementOpeningsV1::new_v1(&mut self.session, &self.table, &self.source, &self.file)
    }
    fn next(&mut self, complements: &mut QMaskComplementOpeningsV1) -> Result<(), QMaskSErrorV1> {
        complements.produce_next_v1(
            &mut self.session,
            &mut self.table,
            &mut self.source,
            &mut self.file,
        )
    }
}
fn completed(
    path: &std::path::Path,
    malformed: Malformed,
    storage: Option<OrderedStorageSessionBudgetV1>,
) -> Completed {
    let mut f = previous::fixture_with_storage(
        path,
        None,
        storage.unwrap_or_else(OrderedStorageSessionBudgetV1::new_v1),
    );
    let mut stream = previous::unqualified_stream_prefix(&mut f);
    // All6400 predecessor S points/rhos are synthetic. This helper only supplies
    // private boundary state for actual file and next-complement tests.
    for i in 4..6400 {
        stream.blindings.push(Scalar::from_u64(i as u64 + 1));
        f.session.inventory.slots[27176 + i] = Some(GlobalLookupCommitmentTicketV1 {
            coordinate: digit_coordinate_v1(
                QMaskSBlockCoordinateV1::from_ordinal_v1(i / 4).unwrap(),
                i % 4,
            )
            .unwrap(),
            point_wire: Point::canonical_generator()
                .unwrap()
                .to_non_identity_wire_bytes()
                .unwrap(),
        });
    }
    stream.block.coordinate = QMaskSBlockCoordinateV1::from_ordinal_v1(1599).unwrap();
    stream.block.coefficients.values.as_mut_slice().zeroize();
    f.session.next_global_ordinal = 33576;
    f.session.next_purpose = GlobalLookupCommitmentPurposeV1::QMaskComplementDigit;
    f.session.next_purpose_ordinal = 0;
    let GlobalLookupProofSessionEntropySourceV1::Production {
        commitment_entropy_bytes,
        q_mask_entropy_bytes,
        ..
    } = &mut f.session.entropy
    else {
        unreachable!()
    };
    *commitment_entropy_bytes = 6400 * 32;
    *q_mask_entropy_bytes = stream.block.coordinate.sampled_values_through_v1() * 8;
    let Fixture {
        session,
        table,
        file: mut written,
        storage,
        ..
    } = f;
    for block in 1..1600 {
        let mut writer = written.resume_next_block_v1().unwrap();
        for local in 0..8 {
            let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16384).unwrap();
            if matches!(malformed, Malformed::Residue) && block == 1 && local == 0 {
                chunk.as_mut_slice_v1()[..8]
                    .copy_from_slice(&ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0].to_le_bytes());
            }
            if matches!(malformed, Malformed::Top) && block == 7 && local == 7 {
                chunk.as_mut_slice_v1()[16376..].copy_from_slice(&1_u64.to_le_bytes());
            }
            writer.write_slot_v1(block * 8 + local, chunk).unwrap();
        }
        written = writer.finish_block_v1().unwrap();
    }
    let source = stream.finish_v1(&session, &table).unwrap();
    let file = written.seal_v1().unwrap();
    Completed {
        session,
        table,
        source,
        file,
        storage,
    }
}
// Skip only fixture arithmetic; every original file block is still authenticated
// and read exactly once. The explicitly synthetic complement prefix is not proof.
fn synthetic_complement_prefix(
    f: &mut Completed,
    c: &mut QMaskComplementOpeningsV1,
    blocks: usize,
) {
    for _ in 0..blocks {
        let block = QMaskSBlockCoordinateV1::from_ordinal_v1(f.source.next_block).unwrap();
        f.source
            .load_next_original_block_v1(&f.session, &mut f.file)
            .unwrap();
        for digit in 0..4 {
            let coordinate = complement_coordinate_v1(block, digit).unwrap();
            f.session.inventory.slots[coordinate.global_ordinal as usize] =
                Some(GlobalLookupCommitmentTicketV1 {
                    coordinate,
                    point_wire: Point::canonical_generator()
                        .unwrap()
                        .to_non_identity_wire_bytes()
                        .unwrap(),
                });
            c.blindings.push(Scalar::from_u64(1));
        }
        f.source.erase_loaded_block_v1().unwrap();
    }
    let count = c.blindings.len() as u32;
    let next = commitment_coordinate_v1(33576 + count).unwrap();
    f.session.next_global_ordinal = next.global_ordinal;
    f.session.next_purpose = next.purpose;
    f.session.next_purpose_ordinal = next.purpose_ordinal;
    let GlobalLookupProofSessionEntropySourceV1::Production {
        commitment_entropy_bytes,
        ..
    } = &mut f.session.entropy
    else {
        unreachable!()
    };
    *commitment_entropy_bytes = c.commitment_entropy_before + u64::from(count) * 32;
}
fn assert_actual_equations(f: &Completed, c: &QMaskComplementOpeningsV1, block: usize) {
    let basis = Suite::generators().reduce(16384).unwrap();
    let sum = basis
        .g_bold
        .iter()
        .copied()
        .fold(Point::identity(), |a, b| a + b);
    let q = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[block / 40];
    for digit in 0..4 {
        let divisor = 32768_u64.pow(digit as u32);
        let baseline = Scalar::from_u64((q - 1) / divisor % 32768);
        let rho = &c.blindings.as_slice()[block * 4 + digit];
        assert_eq!(
            *rho,
            Scalar::from_be_bytes_exact(previous::rho_bytes(digit)).unwrap()
        );
        let mut expected =
            SecretMultiexpBuilder::<Suite>::new(if block == 0 { 6 } else { 2 }).unwrap();
        expected.push(&baseline, &sum).unwrap();
        if block == 0 {
            for index in [0, 255, 256, 16383] {
                let scalar =
                    Scalar::from_u64((q - 1 - previous::coefficient(index)) / divisor % 32768)
                        - baseline;
                expected.push(&scalar, &basis.g_bold[index]).unwrap();
            }
        }
        expected.push(rho, &basis.h).unwrap();
        let coordinate = complement_coordinate_v1(
            QMaskSBlockCoordinateV1::from_ordinal_v1(block).unwrap(),
            digit,
        )
        .unwrap();
        let actual = f.session.inventory.slots[coordinate.global_ordinal as usize]
            .as_ref()
            .unwrap();
        assert_eq!(actual.coordinate, coordinate);
        assert_eq!(
            actual.point_wire,
            expected
                .evaluate()
                .unwrap()
                .expose_ref()
                .to_non_identity_wire_bytes()
                .unwrap()
        );
    }
}
#[test]
fn qmask_complement_exact_inventory_and_multiplicity_boundary() {
    let mut seen = std::collections::BTreeSet::new();
    for ordinal in 0..1600 {
        for digit in 0..4 {
            let c = complement_coordinate_v1(
                QMaskSBlockCoordinateV1::from_ordinal_v1(ordinal).unwrap(),
                digit,
            )
            .unwrap();
            assert_eq!(
                (c.global_ordinal, c.purpose_ordinal),
                (
                    33576 + 4 * ordinal as u32 + digit as u32,
                    4 * ordinal as u32 + digit as u32
                )
            );
            assert!(seen.insert(c.global_ordinal));
        }
    }
    assert_eq!(
        (seen.len(), *seen.first().unwrap(), *seen.last().unwrap()),
        (6400, 33576, 39975)
    );
    let next = commitment_coordinate_v1(39976).unwrap();
    assert_eq!(
        (next.purpose, next.purpose_ordinal),
        (GlobalLookupCommitmentPurposeV1::Multiplicity, 0)
    );
    assert!(
        complement_coordinate_v1(QMaskSBlockCoordinateV1::from_ordinal_v1(0).unwrap(), 4).is_err()
    );
}
#[test]
fn qmask_complement_integer_borrow_all_limbs_and_nonzero_final_coefficient() {
    for q in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1 {
        for s in [
            0,
            1,
            32767,
            32768,
            32769,
            1 << 30,
            (1 << 45) + 7,
            q - 2,
            q - 1,
        ] {
            let mut reconstructed = 0_u64;
            for h in 0..4 {
                let digit = complement_digit_v1(s, q, h).unwrap();
                assert!(digit < 32768);
                reconstructed += u64::from(digit) * 32768_u64.pow(h as u32);
            }
            assert_eq!(reconstructed, q - 1 - s);
        }
        assert_ne!(q - 1, (1 << 60) - 1);
        assert!(complement_digit_v1(q, q, 0).is_err());
        assert!(complement_digit_v1(u64::MAX, q, 0).is_err());
    }
    assert!(complement_digit_v1(0, 0, 0).is_err());
    assert!(complement_digit_v1(0, 1 << 60, 0).is_err());
    assert!(complement_digit_v1(0, 7, 4).is_err());
}
#[test]
#[cfg(unix)]
fn qmask_complement_first_actual_four_equations_original_rhos_and_drop_refund() {
    let dir = TestDirectory::new("qmask-complement-equations");
    let mut f = completed(dir.path(), Malformed::None, None);
    let mut c = f.begin().unwrap();
    let original_drops = std::rc::Rc::clone(&f.random().drops);
    let pointer = f.source.stream.block.coefficients.values.as_ptr();
    let source_rhos = f.source.stream.blindings.as_slice().as_ptr();
    let complement_rhos = c.blindings.as_slice().as_ptr();
    let memory = f.session.proof_resources.live_bytes().unwrap();
    let io = f.storage.test_usage_words_v1()[3];
    controls::reset_v1();
    f.next(&mut c).unwrap();
    assert_actual_equations(&f, &c, 0);
    assert_eq!(f.session.next_global_ordinal, 33580);
    assert_eq!(
        (f.random().mask_calls.get(), f.random().rho_calls.get()),
        (16384, 4)
    );
    assert_eq!(f.source.stream.block.coefficients.values.as_ptr(), pointer);
    assert_eq!(f.source.stream.blindings.as_slice().as_ptr(), source_rhos);
    assert_eq!(c.blindings.as_slice().as_ptr(), complement_rhos);
    assert!(
        f.source
            .stream
            .block
            .coefficients
            .values
            .iter()
            .all(|x| *x == 0)
    );
    assert_eq!(f.session.proof_resources.live_bytes().unwrap(), memory);
    assert_eq!(f.storage.test_usage_words_v1()[3], io + 131200);
    assert_eq!(controls::work_v1().selects, 4194304);
    assert_eq!(controls::clear_v1(), (16384, true));
    assert!(
        c.require_complete_v1(&f.session, &f.table, &f.source, &f.file)
            .is_err()
    );
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    drop(c);
    drop(f.source);
    drop(f.file);
    drop(f.table);
    assert!(zeroizing_t256_scalar_vec_drop_count_v1() >= before + 2);
    assert_eq!(f.session.proof_resources.live_bytes().unwrap(), 0);
    assert_eq!(f.storage.test_usage_words_v1()[0], 0);
    assert_eq!(original_drops.get(), 0);
    drop(f.session);
    assert_eq!(original_drops.get(), 1);
}
#[test]
#[cfg(unix)]
fn qmask_complement_last_actual_equations_complete_only_after_every_original_read() {
    let dir = TestDirectory::new("qmask-complement-last");
    let mut f = completed(dir.path(), Malformed::None, None);
    let mut c = f.begin().unwrap();
    synthetic_complement_prefix(&mut f, &mut c, 1599);
    assert!(
        c.require_complete_v1(&f.session, &f.table, &f.source, &f.file)
            .is_err()
    );
    f.next(&mut c).unwrap();
    assert_actual_equations(&f, &c, 1599);
    assert_eq!(f.session.next_global_ordinal, 39976);
    assert_eq!(
        f.session.next_purpose,
        GlobalLookupCommitmentPurposeV1::Multiplicity
    );
    assert_eq!(f.session.next_purpose_ordinal, 0);
    c.require_complete_v1(&f.session, &f.table, &f.source, &f.file)
        .unwrap();
    assert!(f.next(&mut c).is_err());
    let ticket = f.session.inventory.slots[33576].take().unwrap();
    assert!(
        c.require_complete_v1(&f.session, &f.table, &f.source, &f.file)
            .is_err()
    );
    f.session.inventory.slots[33576] = Some(ticket);
    c.require_complete_v1(&f.session, &f.table, &f.source, &f.file)
        .unwrap();
    // Source top is zero, but its complement is q39-1; independent dense
    // baseline equation above includes the final basis point with that value.
    assert_eq!(f.random().rho_calls.get(), 4);
}
#[test]
#[cfg(unix)]
fn qmask_complement_preallocation_capacity_preserves_all_original_owners() {
    let dir = TestDirectory::new("qmask-complement-capacity");
    let mut f = completed(dir.path(), Malformed::None, None);
    let baseline = f.session.proof_resources.live_bytes().unwrap();
    let retained = (6400 * core::mem::size_of::<Scalar>()
        + core::mem::size_of::<QMaskComplementOpeningsV1>()) as u64;
    let competitor = f
        .session
        .proof_resources
        .reserve_workspace_v1(
            ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1 - baseline - retained + 1,
            0,
        )
        .unwrap();
    assert!(matches!(f.begin(), Err(QMaskSErrorV1::Capacity)));
    assert_eq!(f.session.next_global_ordinal, 33576);
    assert_eq!(f.random().rho_calls.get(), 0);
    drop(competitor);
    let mut c = f.begin().unwrap();
    let admit = ComplementBlockAdmissionV1::new_v1(&mut f.session, &f.table).unwrap();
    let exact = f.session.proof_resources.live_bytes().unwrap();
    drop(admit);
    let prior = f.session.proof_resources.live_bytes().unwrap();
    let blocker = f
        .session
        .proof_resources
        .reserve_workspace_v1(ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1 - exact + 1, 0)
        .unwrap();
    let io = f.storage.test_usage_words_v1();
    let pointer = f.source.stream.block.coefficients.values.as_ptr();
    controls::reset_v1();
    assert!(matches!(f.next(&mut c), Err(QMaskSErrorV1::Capacity)));
    assert_eq!(controls::allocations_v1(), 0);
    assert_eq!(f.storage.test_usage_words_v1(), io);
    assert_eq!(f.source.stream.block.coefficients.values.as_ptr(), pointer);
    assert_eq!(f.source.next_block, 0);
    assert_eq!(f.random().rho_calls.get(), 0);
    assert_eq!(c.blindings.len(), 0);
    drop(blocker);
    assert_eq!(f.session.proof_resources.live_bytes().unwrap(), prior);
    let retry = ComplementBlockAdmissionV1::new_v1(&mut f.session, &f.table).unwrap();
    assert_eq!(f.session.proof_resources.live_bytes().unwrap(), exact);
    drop(retry);
}
#[test]
#[cfg(unix)]
fn qmask_complement_io_capacity_preserves_then_retries_original_block() {
    let dir = TestDirectory::new("qmask-complement-io-capacity");
    let pair = 66 * 16400;
    let s = 209920000;
    let storage = OrderedStorageSessionBudgetV1::with_test_limits_v1(s + pair, 2 * s + 4 * pair);
    let mut f = completed(dir.path(), Malformed::None, Some(storage));
    let mut c = f.begin().unwrap();
    let competitor =
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(dir.path(), [93; 32], &mut f.storage)
            .unwrap();
    let io = f.storage.test_usage_words_v1();
    let memory = f.session.proof_resources.live_bytes().unwrap();
    assert!(matches!(f.next(&mut c), Err(QMaskSErrorV1::Capacity)));
    assert_eq!(f.storage.test_usage_words_v1(), io);
    assert_eq!(f.session.proof_resources.live_bytes().unwrap(), memory);
    assert_eq!(
        (
            f.source.next_block,
            c.blindings.len(),
            f.random().rho_calls.get()
        ),
        (0, 0, 0)
    );
    f.file.require_next_block_v1(0).unwrap();
    drop(competitor);
    // A late RNG failure stops the retry after its real successful first read,
    // avoiding an unnecessary repeated four-MSM equation case.
    f.random().fault.set(Fault::ErrorAt(0));
    assert!(matches!(f.next(&mut c), Err(QMaskSErrorV1::Entropy)));
    assert_eq!(f.random().rho_calls.get(), 1);
    assert_eq!(f.storage.test_usage_words_v1()[3], io[3] + 131200);
    f.file.require_next_block_v1(1).unwrap();
}
#[test]
#[cfg(unix)]
fn qmask_complement_rejects_context_foreign_ledger_order_and_reused_ticket_before_io() {
    let dir = TestDirectory::new("qmask-complement-context");
    let mut f = completed(dir.path(), Malformed::None, None);
    let io = f.storage.test_usage_words_v1();
    let binding = f.source.stream.block.binding;
    f.source.stream.block.binding[0] ^= 1;
    assert!(f.begin().is_err());
    f.source.stream.block.binding = binding;
    let other = crate::vega::zk_ams::mkhe::rns_native_resource_budget::RnsNativeProofResourceBudgetV1::default();
    let original = std::mem::replace(&mut f.session.proof_resources, other);
    assert!(f.begin().is_err());
    f.session.proof_resources = original;
    let mut c = f.begin().unwrap();
    for ordinal in [33575, 33577, 33580, 39976] {
        f.session.next_global_ordinal = ordinal;
        assert!(f.next(&mut c).is_err());
    }
    f.session.next_global_ordinal = 33576;
    f.session.next_purpose = GlobalLookupCommitmentPurposeV1::QMaskDigit;
    assert!(f.next(&mut c).is_err());
    f.session.next_purpose = GlobalLookupCommitmentPurposeV1::QMaskComplementDigit;
    f.session.inventory.slots[33579] = Some(GlobalLookupCommitmentTicketV1 {
        coordinate: commitment_coordinate_v1(33579).unwrap(),
        point_wire: Point::canonical_generator()
            .unwrap()
            .to_non_identity_wire_bytes()
            .unwrap(),
    });
    assert!(f.next(&mut c).is_err());
    f.session.inventory.slots[33579] = None;
    let GlobalLookupProofSessionEntropySourceV1::Production {
        q_mask_entropy_bytes,
        ..
    } = &mut f.session.entropy
    else {
        unreachable!()
    };
    *q_mask_entropy_bytes += 8;
    assert!(f.next(&mut c).is_err());
    assert_eq!(f.storage.test_usage_words_v1(), io);
    assert_eq!(f.random().rho_calls.get(), 0);
    assert_eq!(c.blindings.len(), 0);
}
#[test]
#[cfg(unix)]
fn qmask_complement_authenticated_malformed_residue_and_source_top_fail_before_rho() {
    let dir = TestDirectory::new("qmask-complement-malformed-read");
    for (malformed, prior) in [(Malformed::Residue, 1), (Malformed::Top, 7)] {
        let mut f = completed(dir.path(), malformed, None);
        let mut c = f.begin().unwrap();
        synthetic_complement_prefix(&mut f, &mut c, prior);
        let ordinal = f.session.next_global_ordinal;
        assert!(matches!(f.next(&mut c), Err(QMaskSErrorV1::Source)));
        assert_eq!(f.random().rho_calls.get(), 0);
        assert_eq!(f.session.next_global_ordinal, ordinal);
        assert_eq!(c.blindings.len(), prior * 4);
        assert!(f.file.require_next_block_v1(prior).is_err());
        let before = ZEROIZED_MASK_VALUES_V1.with(Cell::get);
        drop(f.source);
        assert_eq!(ZEROIZED_MASK_VALUES_V1.with(Cell::get), before + 16384);
    }
}
#[test]
#[cfg(unix)]
fn qmask_complement_zero_partial_error_and_unwind_charge_original_entropy_and_erase() {
    let dir = TestDirectory::new("qmask-complement-entropy");
    for fault in [Fault::Zero, Fault::ErrorAt(1), Fault::PanicAt(1)] {
        let mut f = completed(dir.path(), Malformed::None, None);
        let mut c = f.begin().unwrap();
        let before_entropy = entropy_counters_v1(&f.session).unwrap();
        f.random().fault.set(fault);
        let before_drop = zeroizing_t256_scalar_vec_drop_count_v1();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f.next(&mut c)));
        assert_eq!(result.is_err(), matches!(fault, Fault::PanicAt(_)));
        assert!(matches!(result, Err(_) | Ok(Err(QMaskSErrorV1::Entropy))));
        let calls = if matches!(fault, Fault::Zero) { 128 } else { 2 };
        assert_eq!(f.random().rho_calls.get(), calls);
        let counters = entropy_counters_v1(&f.session).unwrap();
        assert_eq!(
            counters,
            (before_entropy.0 + calls as u64 * 32, before_entropy.1)
        );
        let completed = usize::from(!matches!(fault, Fault::Zero));
        assert_eq!(f.session.next_global_ordinal, 33576 + completed as u32);
        assert_eq!(c.blindings.len(), completed);
        assert!(f.next(&mut c).is_err());
        drop(c);
        drop(f.source);
        drop(f.file);
        drop(f.table);
        assert!(zeroizing_t256_scalar_vec_drop_count_v1() >= before_drop + 2);
        assert_eq!(f.session.proof_resources.live_bytes().unwrap(), 0);
    }
}
#[test]
#[cfg(unix)]
fn qmask_complement_late_digit_allocation_failure_is_terminal_after_original_read() {
    let dir = TestDirectory::new("qmask-complement-late-allocation");
    let mut f = completed(dir.path(), Malformed::None, None);
    let mut c = f.begin().unwrap();
    controls::reset_v1();
    controls::fail_digits_v1();
    assert!(matches!(f.next(&mut c), Err(QMaskSErrorV1::Resource)));
    assert_eq!(f.random().rho_calls.get(), 1);
    assert_eq!(f.session.next_global_ordinal, 33576);
    assert_eq!(c.blindings.len(), 0);
    assert!(f.source.loaded);
    assert!(f.next(&mut c).is_err());
    assert_eq!(controls::work_v1().rho_terms, 0);
}
#[test]
#[cfg(unix)]
fn qmask_complement_rejects_missing_source_ticket_after_authentic_read_without_rho() {
    let dir = TestDirectory::new("qmask-complement-source-ticket");
    let mut f = completed(dir.path(), Malformed::None, None);
    let mut c = f.begin().unwrap();
    f.session.inventory.slots[27176] = None;
    assert!(matches!(f.next(&mut c), Err(QMaskSErrorV1::Source)));
    assert_eq!(f.random().rho_calls.get(), 0);
    assert_eq!(f.session.next_global_ordinal, 33576);
    assert!(f.file.require_next_block_v1(0).is_err());
}

#[cfg(unix)]
pub(in super::super::super::super) fn with_complement_for_retained_refusal_v1(
    check: impl FnOnce(
        &mut CompleteQMaskSOpeningsV1,
        &mut SealedQMaskSFileV1,
        &mut QMaskComplementOpeningsV1,
    ),
) {
    let dir = TestDirectory::new("qmask-complement-retained-refusal");
    let mut f = completed(dir.path(), Malformed::None, None);
    let mut complements = f.begin().unwrap();
    let io = f.storage.test_usage_words_v1();
    check(&mut f.source, &mut f.file, &mut complements);
    assert_eq!(f.random().rho_calls.get(), 0);
    assert_eq!(f.storage.test_usage_words_v1(), io);
    assert_eq!(f.source.next_block, 0);
    assert_eq!(complements.blindings.len(), 0);
}
#[test]
fn qmask_complement_named_owner_exposes_no_parallel_replay_or_raw_opening_path() {
    let owner = include_str!("../../retained_source_session_v1.rs");
    let compact = owner.split_whitespace().collect::<String>();
    for (name, call) in [
        (
            "begin_q_mask_complements_v1",
            ".begin_q_mask_complements_v1",
        ),
        (
            "produce_q_mask_complement_block_v1",
            ".produce_q_mask_complement_block_v1",
        ),
        (
            "finish_q_mask_complements_v1",
            ".finish_q_mask_complements_v1",
        ),
    ] {
        let segment = compact.split(&format!("fn{name}(")).nth(1).unwrap();
        assert!(segment.find("self.phase.take()").unwrap() < segment.find(call).unwrap());
    }
    let lower = include_str!("../first_openings_v1.rs");
    assert!(!lower.contains("fn read_next_v1"));
    assert!(!lower.contains("fn advance_read_v1"));
    assert!(lower.contains("fn load_next_original_block_v1"));
    let producer = include_str!("complement_v1.rs");
    for forbidden in [
        "impl Clone",
        "impl Copy",
        "fn into_parts",
        "blinding: &",
        "point: &",
        "FnMut",
    ] {
        assert!(
            !producer.contains(forbidden),
            "unexpected producer surface: {forbidden}"
        );
    }
}

#[test]
#[cfg(unix)]
fn qmask_complement_original_entropy_ceiling_refuses_before_another_rng_request() {
    let dir = TestDirectory::new("qmask-complement-entropy-cap");
    let mut f = completed(dir.path(), Malformed::None, None);
    let GlobalLookupProofSessionEntropySourceV1::Production {
        commitment_entropy_bytes,
        ..
    } = &mut f.session.entropy
    else {
        unreachable!()
    };
    *commitment_entropy_bytes = MAX_COMMITMENT_ENTROPY_BYTES_V1;
    let mut c = f.begin().unwrap();
    assert!(matches!(f.next(&mut c), Err(QMaskSErrorV1::Resource)));
    assert_eq!(f.random().rho_calls.get(), 0);
    assert_eq!(
        entropy_counters_v1(&f.session).unwrap().0,
        MAX_COMMITMENT_ENTROPY_BYTES_V1
    );
    assert_eq!(f.session.next_global_ordinal, 33576);
    assert_eq!(c.blindings.len(), 0);
    assert!(f.source.loaded);
    assert!(f.next(&mut c).is_err());
}

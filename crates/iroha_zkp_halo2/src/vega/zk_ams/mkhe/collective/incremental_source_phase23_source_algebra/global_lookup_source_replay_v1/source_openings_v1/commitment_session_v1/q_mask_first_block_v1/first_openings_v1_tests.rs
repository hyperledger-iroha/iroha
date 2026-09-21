//! Actual four-ticket equations with explicitly unqualified prior source axes.
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
                OrderedPlaneSpoolSnapshotV1, OrderedPlaneSpoolWriterV1,
                OrderedStorageSessionBudgetV1,
            },
            rns_native_resource_budget::RnsNativeProofResourceBudgetV1,
        },
    },
};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy)]
pub(super) enum Fault {
    None,
    Zero,
    ErrorAt(usize),
    PanicAt(usize),
}
pub(super) struct Random {
    pub(super) mask_calls: Rc<Cell<usize>>,
    pub(super) rho_calls: Rc<Cell<usize>>,
    pub(super) drops: Rc<Cell<usize>>,
    pub(super) fault: Rc<Cell<Fault>>,
    pub(super) mask_limit: Rc<Cell<usize>>,
    pub(super) limb: Rc<Cell<usize>>,
}
pub(super) fn coefficient(index: usize) -> u64 {
    match index {
        0 => 1,
        255 => ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0] - 1,
        256 => (1 << 45) + (123 << 30) + (17 << 15) + 9,
        16_383 => (16383 << 45) + (23456 << 30) + (30000 << 15) + 12345,
        _ => 0,
    }
}
pub(super) fn rho_bytes(index: usize) -> [u8; 32] {
    let mut bytes = [0; 32];
    bytes[0] = 0x40;
    bytes[31] = 17_u8.wrapping_add(index as u8);
    bytes
}
impl MaskedRelaxedRandomSourceV1 for Random {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        match destination.len() {
            8 => {
                let index = self.mask_calls.get();
                assert!(
                    index < self.mask_limit.get(),
                    "no unadmitted S block may be read"
                );
                self.mask_calls.set(index + 1);
                let local = index % BLOCK_COEFFICIENTS_V1;
                let value = if local == 255 {
                    ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[self.limb.get()] - 1
                } else {
                    coefficient(local)
                };
                destination.copy_from_slice(&value.to_le_bytes());
            }
            32 => {
                let index = self.rho_calls.get();
                self.rho_calls.set(index + 1);
                match self.fault.get() {
                    Fault::Zero => {
                        destination.fill(0);
                        return Ok(());
                    }
                    Fault::ErrorAt(at) if at == index => {
                        destination[..7].fill(91);
                        return Err(MaskedRelaxedRandomErrorV1::Unavailable);
                    }
                    Fault::PanicAt(at) if at == index => {
                        destination[..7].fill(91);
                        panic!("partial original rho fill unwinds");
                    }
                    _ => {}
                }
                destination.copy_from_slice(&rho_bytes(index));
            }
            _ => panic!("unexpected original entropy width"),
        }
        Ok(())
    }
}
impl Drop for Random {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
pub(super) struct Fixture {
    pub(super) session: GlobalLookupCommitmentSessionLiveV1<Random>,
    pub(super) table: RnsNativeU15MsmTableV1,
    pub(super) block: Option<SampledQMaskSBlockV1>,
    pub(super) file: WrittenQMaskSBlockFileV1,
    pub(super) storage: OrderedStorageSessionBudgetV1,
}
fn pair(
    directory: &std::path::Path,
    budget: &mut OrderedStorageSessionBudgetV1,
) -> OrderedPlaneSpoolSnapshotV1 {
    let mut writer =
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(directory, [73; 32], budget).unwrap();
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
    writer.seal_v1().unwrap()
}
pub(super) fn fixture(directory: &std::path::Path, limit: Option<u64>) -> Fixture {
    fixture_with_storage(directory, limit, OrderedStorageSessionBudgetV1::new_v1())
}
pub(super) fn fixture_with_storage(
    directory: &std::path::Path,
    limit: Option<u64>,
    mut storage: OrderedStorageSessionBudgetV1,
) -> Fixture {
    let pair = pair(directory, &mut storage);
    let plan = pair.q_mask_s_file_plan_v1().unwrap();
    let resources = limit.map_or_else(
        RnsNativeProofResourceBudgetV1::default,
        RnsNativeProofResourceBudgetV1::with_test_workspace_limit_v1,
    );
    // This direct original-entropy fixture deliberately has no authenticated
    // prior source or source prefix. Actual earlier authority stays unavailable.
    let mut session = GlobalLookupCommitmentSessionLiveV1 {
        proof_resources: resources,
        entropy: GlobalLookupProofSessionEntropySourceV1::Production {
            original_random: Random {
                mask_calls: Rc::new(Cell::new(0)),
                rho_calls: Rc::new(Cell::new(0)),
                drops: Rc::new(Cell::new(0)),
                fault: Rc::new(Cell::new(Fault::None)),
                mask_limit: Rc::new(Cell::new(BLOCK_COEFFICIENTS_V1)),
                limb: Rc::new(Cell::new(0)),
            },
            commitment_entropy_bytes: 0,
            q_mask_entropy_bytes: 0,
        },
        inventory: GlobalLookupCommitmentInventorySkeletonV1::new_v1().unwrap(),
        proof_session_context_digest: [71; 32],
        source_opening_context_digest: Some([72; 32]),
        next_global_ordinal: QMASK_FIRST_INVENTORY_V1,
        next_purpose: GlobalLookupCommitmentPurposeV1::QMaskDigit,
        next_purpose_ordinal: 0,
        pending_source: None,
    };
    let table = RnsNativeU15MsmTableV1::new_v1(&mut session.proof_resources).unwrap();
    let (memory, file_memory) = QMaskFirstBlockMemoryV1::new_v1(&mut session, &plan).unwrap();
    let mut file = pair
        .create_q_mask_s_file_v1(directory, plan, file_memory)
        .unwrap();
    let block = SampledQMaskSBlockV1::sample_v1(&mut session, memory).unwrap();
    block.write_slots_v1(&mut file).unwrap();
    let file = file.finish_block_v1().unwrap();
    Fixture {
        session,
        table,
        block: Some(block),
        file,
        storage,
    }
}
impl Fixture {
    pub(super) fn random(&self) -> &Random {
        match &self.session.entropy {
            GlobalLookupProofSessionEntropySourceV1::Production {
                original_random, ..
            } => original_random,
            _ => unreachable!(),
        }
    }
    fn admit(&mut self) -> Result<QMaskSBlockAdmissionV1, QMaskSErrorV1> {
        QMaskSBlockAdmissionV1::new_v1(
            &mut self.session,
            &self.table,
            self.block.as_ref().unwrap(),
            &self.file,
        )
    }
    fn produce(
        &mut self,
        admission: QMaskSBlockAdmissionV1,
    ) -> Result<QMaskSOpeningStreamV1, QMaskSErrorV1> {
        QMaskSOpeningStreamV1::produce_v1(
            &mut self.session,
            &mut self.table,
            self.block.take().unwrap(),
            admission,
        )
    }
}
#[test]
#[cfg(unix)]
fn qmask_four_openings_install_exact_true_generator_equations_and_original_rhos() {
    let directory = TestDirectory::new("qmask-four-equations");
    let mut fixture = fixture(directory.path(), None);
    let drops = Rc::clone(&fixture.random().drops);
    let admission = fixture.admit().unwrap();
    assert_eq!(fixture.random().rho_calls.get(), 0);
    controls::reset_v1();
    let opened = fixture.produce(admission).unwrap();
    assert_eq!(fixture.random().mask_calls.get(), 16_384);
    assert_eq!(fixture.random().rho_calls.get(), 4);
    assert_eq!(fixture.session.next_global_ordinal, 27_180);
    assert_eq!(fixture.session.next_purpose_ordinal, 4);
    assert_eq!(
        fixture.session.next_purpose,
        GlobalLookupCommitmentPurposeV1::QMaskDigit
    );
    assert_eq!(opened.blindings.len(), 4);
    let work = controls::work_v1();
    assert_eq!(
        (
            work.windows,
            work.doubles,
            work.selects,
            work.window_adds,
            work.folds,
            work.rho_terms,
            work.combines
        ),
        (1024, 4096, 4_194_304, 262_144, 256, 4, 4)
    );
    assert_eq!(controls::clear_v1(), (16_384, true));
    let basis = Suite::generators().reduce(16_384).unwrap();
    for digit in 0..4 {
        let rho = &opened.blindings.as_slice()[digit];
        assert_eq!(*rho, Scalar::from_be_bytes_exact(rho_bytes(digit)).unwrap());
        let mut expected = SecretMultiexpBuilder::<Suite>::new(5).unwrap();
        for index in [0, 255, 256, 16_383] {
            let value = coefficient(index) / 32768_u64.pow(digit as u32) % 32768;
            expected
                .push(&Scalar::from_u64(value), &basis.g_bold[index])
                .unwrap();
        }
        expected.push(rho, &basis.h).unwrap();
        let expected = expected.evaluate().unwrap();
        let ticket = fixture.session.inventory.slots[27_176 + digit]
            .as_ref()
            .unwrap();
        assert_eq!(ticket.coordinate, first_digit_coordinate_v1(digit).unwrap());
        assert_eq!(
            ticket.point_wire,
            expected.expose_ref().to_non_identity_wire_bytes().unwrap()
        );
        assert_eq!(opened.block.coefficients.values[256], coefficient(256));
    }
    assert!(
        fixture.session.inventory.slots[..27_176]
            .iter()
            .all(Option::is_none)
    );
    assert!(
        fixture.session.inventory.slots[27_180..]
            .iter()
            .all(Option::is_none)
    );
    assert!(matches!(
        QMaskSBlockAdmissionV1::new_v1(
            &mut fixture.session,
            &fixture.table,
            &opened.block,
            &fixture.file
        ),
        Err(QMaskSErrorV1::Source)
    ));
    assert_eq!(fixture.random().rho_calls.get(), 4);
    let GlobalLookupProofSessionEntropySourceV1::Production {
        commitment_entropy_bytes,
        q_mask_entropy_bytes,
        ..
    } = &fixture.session.entropy
    else {
        unreachable!()
    };
    assert_eq!(
        (*commitment_entropy_bytes, *q_mask_entropy_bytes),
        (128, 131_072)
    );
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    drop(opened);
    assert!(zeroizing_t256_scalar_vec_drop_count_v1() > before);
    drop(fixture.file);
    drop(fixture.table);
    assert_eq!(fixture.session.proof_resources.live_bytes().unwrap(), 0);
    assert_eq!(fixture.storage.test_usage_words_v1()[0], 0);
    drop(fixture.session);
    assert_eq!(drops.get(), 1);
}
#[test]
#[cfg(unix)]
fn qmask_four_openings_capacity_preserves_original_preimage_and_retries_after_actual_release() {
    let directory = TestDirectory::new("qmask-four-capacity");
    let mut probe = fixture(directory.path(), None);
    let admission = probe.admit().unwrap();
    let exact = probe.session.proof_resources.live_bytes().unwrap();
    drop(admission);
    drop(probe);
    let mut fixture = fixture(directory.path(), Some(exact));
    let pointer = fixture.block.as_ref().unwrap().coefficients.values.as_ptr();
    let identity = fixture.file.require_block_binding_v1().unwrap();
    let prior = fixture.session.proof_resources.live_bytes().unwrap();
    let competitor = fixture
        .session
        .proof_resources
        .reserve_workspace_v1(1, 0)
        .unwrap();
    controls::reset_v1();
    assert!(matches!(fixture.admit(), Err(QMaskSErrorV1::Capacity)));
    assert_eq!(controls::allocations_v1(), 0);
    assert_eq!(
        fixture.session.proof_resources.live_bytes().unwrap(),
        prior + 1
    );
    assert_eq!(fixture.random().rho_calls.get(), 0);
    assert_eq!(fixture.session.next_global_ordinal, 27_176);
    assert_eq!(
        fixture.block.as_ref().unwrap().coefficients.values.as_ptr(),
        pointer
    );
    assert_eq!(fixture.file.require_block_binding_v1().unwrap(), identity);
    drop(competitor);
    let retry = fixture.admit().unwrap();
    assert_eq!(fixture.session.proof_resources.live_bytes().unwrap(), exact);
    assert_eq!(fixture.random().rho_calls.get(), 0);
    drop(retry);
    assert_eq!(fixture.session.proof_resources.live_bytes().unwrap(), prior);
}
#[test]
#[cfg(unix)]
fn qmask_four_openings_reject_foreign_preimage_file_table_and_admission_before_rho() {
    let directory = TestDirectory::new("qmask-four-foreign");
    let mut first = fixture(directory.path(), None);
    let mut other = fixture(directory.path(), None);
    assert!(matches!(
        QMaskSBlockAdmissionV1::new_v1(
            &mut first.session,
            &first.table,
            other.block.as_ref().unwrap(),
            &first.file
        ),
        Err(QMaskSErrorV1::Source)
    ));
    assert!(matches!(
        QMaskSBlockAdmissionV1::new_v1(
            &mut first.session,
            &first.table,
            first.block.as_ref().unwrap(),
            &other.file
        ),
        Err(QMaskSErrorV1::Source)
    ));
    assert!(matches!(
        QMaskSBlockAdmissionV1::new_v1(
            &mut first.session,
            &other.table,
            first.block.as_ref().unwrap(),
            &first.file
        ),
        Err(QMaskSErrorV1::Source)
    ));
    let foreign = other.admit().unwrap();
    assert!(matches!(first.produce(foreign), Err(QMaskSErrorV1::Source)));
    assert_eq!(first.random().rho_calls.get(), 0);
    assert_eq!(other.random().rho_calls.get(), 0);
    assert!(
        first.session.inventory.slots[27_176..]
            .iter()
            .all(Option::is_none)
    );
}
#[test]
#[cfg(unix)]
fn qmask_four_openings_reject_skipped_replayed_role_and_entropy_coordinates() {
    let directory = TestDirectory::new("qmask-four-coordinate");
    let mut fixture = fixture(directory.path(), None);
    for ordinal in [0, 27_175, 27_177, 27_180] {
        fixture.session.next_global_ordinal = ordinal;
        assert!(matches!(fixture.admit(), Err(QMaskSErrorV1::Source)));
    }
    fixture.session.next_global_ordinal = 27_176;
    fixture.session.next_purpose = GlobalLookupCommitmentPurposeV1::QMaskComplementDigit;
    assert!(matches!(fixture.admit(), Err(QMaskSErrorV1::Source)));
    fixture.session.next_purpose = GlobalLookupCommitmentPurposeV1::QMaskDigit;
    fixture.session.next_purpose_ordinal = 1;
    assert!(matches!(fixture.admit(), Err(QMaskSErrorV1::Source)));
    fixture.session.next_purpose_ordinal = 0;
    fixture.session.inventory.slots[27_179] = Some(GlobalLookupCommitmentTicketV1 {
        coordinate: first_digit_coordinate_v1(3).unwrap(),
        point_wire: Point::canonical_generator()
            .unwrap()
            .to_non_identity_wire_bytes()
            .unwrap(),
    });
    assert!(matches!(fixture.admit(), Err(QMaskSErrorV1::Source)));
    fixture.session.inventory.slots[27_179] = None;
    fixture.session.inventory.slots.truncate(27_179);
    assert!(matches!(fixture.admit(), Err(QMaskSErrorV1::Source)));
    fixture.session.inventory.slots.resize_with(
        GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1 as usize,
        || None,
    );
    let admission = fixture.admit().unwrap();
    let GlobalLookupProofSessionEntropySourceV1::Production {
        commitment_entropy_bytes,
        ..
    } = &mut fixture.session.entropy
    else {
        unreachable!()
    };
    *commitment_entropy_bytes += 32;
    assert!(matches!(
        fixture.produce(admission),
        Err(QMaskSErrorV1::Source)
    ));
    assert_eq!(fixture.random().rho_calls.get(), 0);
    assert!(first_digit_coordinate_v1(4).is_err());
}
#[test]
#[cfg(unix)]
fn qmask_four_openings_reject_malformed_original_coefficients_without_truncation() {
    let directory = TestDirectory::new("qmask-four-value");
    let mut fixture = fixture(directory.path(), None);
    let admission = fixture.admit().unwrap();
    fixture.block.as_mut().unwrap().coefficients.values[16_383] =
        ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0];
    controls::reset_v1();
    assert!(matches!(
        fixture.produce(admission),
        Err(QMaskSErrorV1::Source)
    ));
    assert_eq!(fixture.random().rho_calls.get(), 0);
    assert_eq!(controls::allocations_v1(), 0);
    assert!(
        fixture.session.inventory.slots[27_176..]
            .iter()
            .all(Option::is_none)
    );
}
#[test]
#[cfg(unix)]
fn qmask_four_openings_zero_rho_exhaustion_and_digit_allocation_failure_consume_preimage() {
    let directory = TestDirectory::new("qmask-four-rho-refusal");
    for zero in [true, false] {
        let mut fixture = fixture(directory.path(), None);
        let admission = fixture.admit().unwrap();
        controls::reset_v1();
        if zero {
            fixture.random().fault.set(Fault::Zero);
        } else {
            controls::fail_digits_v1();
        }
        let before = zeroizing_t256_scalar_vec_drop_count_v1();
        let result = fixture.produce(admission);
        assert!(matches!(
            result,
            Err(QMaskSErrorV1::Entropy | QMaskSErrorV1::Resource)
        ));
        assert!(fixture.block.is_none());
        assert!(zeroizing_t256_scalar_vec_drop_count_v1() > before);
        assert_eq!(fixture.random().rho_calls.get(), if zero { 128 } else { 1 });
        assert_eq!(controls::work_v1().rho_terms, 0);
        assert!(
            fixture.session.inventory.slots[27_176..]
                .iter()
                .all(Option::is_none)
        );
        let GlobalLookupProofSessionEntropySourceV1::Production {
            commitment_entropy_bytes,
            q_mask_entropy_bytes,
            ..
        } = &fixture.session.entropy
        else {
            unreachable!()
        };
        assert_eq!(*commitment_entropy_bytes, if zero { 4096 } else { 32 });
        assert_eq!(*q_mask_entropy_bytes, 131_072);
    }
}
#[test]
#[cfg(unix)]
fn qmask_four_openings_partial_rho_error_and_unwind_erase_already_retained_opening() {
    let directory = TestDirectory::new("qmask-four-partial");
    for panic in [false, true] {
        let mut fixture = fixture(directory.path(), None);
        fixture.random().fault.set(if panic {
            Fault::PanicAt(1)
        } else {
            Fault::ErrorAt(1)
        });
        let admission = fixture.admit().unwrap();
        let before = zeroizing_t256_scalar_vec_drop_count_v1();
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fixture.produce(admission)));
        assert_eq!(result.is_err(), panic);
        assert!(matches!(result, Err(_) | Ok(Err(QMaskSErrorV1::Entropy))));
        assert!(fixture.block.is_none());
        assert!(zeroizing_t256_scalar_vec_drop_count_v1() > before);
        assert_eq!(fixture.random().rho_calls.get(), 2);
        assert_eq!(fixture.session.next_global_ordinal, 27_177);
        assert!(fixture.session.inventory.slots[27_176].is_some());
        assert!(
            fixture.session.inventory.slots[27_177..]
                .iter()
                .all(Option::is_none)
        );
        // Only this hostile lower fixture can still inspect a partially filled
        // session; the actual retained phase is taken before this call.
        drop(fixture);
    }
}

// The retained-dispatch negative control receives real sampled/file/admission
// owners but can only reject them in its independently constructed earlier phase.
#[cfg(unix)]
pub(in super::super::super) fn with_first_openings_for_retained_refusal_v1(
    check: impl FnOnce(SampledQMaskSBlockV1, WrittenQMaskSBlockFileV1, QMaskSBlockAdmissionV1),
) {
    let directory = TestDirectory::new("qmask-first-retained-refusal");
    let mut fixture = fixture(directory.path(), None);
    let admission = fixture.admit().unwrap();
    let block = fixture.block.take().unwrap();
    let rho_calls = Rc::clone(&fixture.random().rho_calls);
    let before = ZEROIZED_MASK_VALUES_V1.with(Cell::get);
    check(block, fixture.file, admission);
    assert_eq!(rho_calls.get(), 0);
    assert_eq!(
        ZEROIZED_MASK_VALUES_V1.with(Cell::get),
        before + BLOCK_COEFFICIENTS_V1
    );
}

#[test]
fn qmask_s_stream_coordinates_cover_every_actual_ticket_and_top_boundary_once() {
    let mut seen = std::collections::BTreeSet::new();
    for ordinal in 0..1600 {
        let c = QMaskSBlockCoordinateV1::from_ordinal_v1(ordinal).unwrap();
        assert_eq!((c.limb * 5 + c.repetition) * 8 + c.block, ordinal);
        assert!(c.limb < 40 && c.repetition < 5 && c.block < 8);
        for digit in 0..4 {
            let ticket = digit_coordinate_v1(c, digit).unwrap();
            assert_eq!(
                ticket.global_ordinal,
                27176 + 4 * ordinal as u32 + digit as u32
            );
            assert!(seen.insert(ticket.global_ordinal));
        }
    }
    assert_eq!(seen.len(), 6400);
    assert_eq!(
        (*seen.first().unwrap(), *seen.last().unwrap()),
        (27176, 33575)
    );
    for (ordinal, axes, sampled) in [
        (0, (0, 0, 0), 16384),
        (7, (0, 0, 7), 131071),
        (8, (0, 1, 0), 147455),
        (39, (0, 4, 7), 655355),
        (40, (1, 0, 0), 671739),
        (1599, (39, 4, 7), 26214200),
    ] {
        let c = QMaskSBlockCoordinateV1::from_ordinal_v1(ordinal).unwrap();
        assert_eq!((c.limb, c.repetition, c.block), axes);
        assert_eq!(c.sampled_values_through_v1(), sampled);
    }
    assert!(QMaskSBlockCoordinateV1::from_ordinal_v1(1600).is_err());
    let next = commitment_coordinate_v1(33576).unwrap();
    assert_eq!(
        (next.purpose, next.purpose_ordinal),
        (GlobalLookupCommitmentPurposeV1::QMaskComplementDigit, 0)
    );
}
#[test]
fn qmask_s_stream_same_allocation_refills_canonical_limbs_and_skips_each_top_draw() {
    struct ZeroRandom {
        calls: usize,
    }
    impl MaskedRelaxedRandomSourceV1 for ZeroRandom {
        fn fill_bytes(&mut self, out: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            assert_eq!(out.len(), 8);
            self.calls += 1;
            out.fill(0);
            Ok(())
        }
    }
    let mut random = ZeroRandom { calls: 0 };
    let mut values = sample_block_v1(&mut random, 0, 0).unwrap();
    let pointer = values.values.as_ptr();
    for ordinal in [7, 8, 39, 40, 1599] {
        let c = QMaskSBlockCoordinateV1::from_ordinal_v1(ordinal).unwrap();
        values.values.fill(u64::MAX);
        let before = random.calls;
        sample_block_into_v1(&mut random, c.limb, c.block, &mut values).unwrap();
        assert_eq!(random.calls - before, 16384 - usize::from(c.block == 7));
        assert_eq!(values.values.as_ptr(), pointer);
        assert_eq!(values.values.len(), 16384);
        assert!(values.values.iter().all(|x| *x == 0));
    }
    let before = random.calls;
    assert!(sample_block_into_v1(&mut random, 40, 0, &mut values).is_err());
    assert!(sample_block_into_v1(&mut random, 0, 8, &mut values).is_err());
    assert_eq!(random.calls, before);
}

// Deliberately unqualified predecessor: actual S file/RNG/table and allocations,
// but synthetic previous point tickets. This is only a next-block boundary fixture.
pub(super) fn unqualified_stream_prefix(fixture: &mut Fixture) -> QMaskSOpeningStreamV1 {
    let mut admission = fixture.admit().unwrap();
    let reservation = admission.rho_owner.take().unwrap();
    drop(admission);
    let mut blindings = ZeroizingT256ScalarVecV1::try_with_exact_capacity(6400).unwrap();
    for digit in 0..4 {
        blindings.push(Scalar::from_u64(digit as u64 + 1));
        fixture.session.inventory.slots[27176 + digit] = Some(GlobalLookupCommitmentTicketV1 {
            coordinate: first_digit_coordinate_v1(digit).unwrap(),
            point_wire: Point::canonical_generator()
                .unwrap()
                .to_non_identity_wire_bytes()
                .unwrap(),
        });
    }
    fixture.session.next_global_ordinal = 27180;
    fixture.session.next_purpose_ordinal = 4;
    QMaskSOpeningStreamV1 {
        block: fixture.block.take().unwrap(),
        blindings,
        reservation,
    }
}
#[test]
#[cfg(unix)]
fn qmask_s_stream_next_block_uses_same_rng_buffer_and_four_actual_original_equations() {
    let directory = TestDirectory::new("qmask-s-stream-next");
    let mut f = fixture(directory.path(), None);
    let stream = unqualified_stream_prefix(&mut f);
    let pointer = stream.block.coefficients.values.as_ptr();
    f.random().mask_limit.set(32768);
    let admission =
        QMaskSBlockAdmissionV1::next_v1(&mut f.session, &f.table, &stream, &f.file).unwrap();
    let mut file = f.file.resume_next_block_v1().unwrap();
    controls::reset_v1();
    let stream = stream
        .continue_v1(&mut f.session, &mut f.table, &mut file, admission)
        .unwrap();
    let file = file.finish_block_v1().unwrap();
    assert_eq!(file.block_ordinal_v1().unwrap(), 1);
    assert_eq!(stream.block.coefficients.values.as_ptr(), pointer);
    assert_eq!(stream.blindings.len(), 8);
    assert_eq!(f.session.next_global_ordinal, 27184);
    let GlobalLookupProofSessionEntropySourceV1::Production {
        original_random,
        commitment_entropy_bytes,
        q_mask_entropy_bytes,
    } = &f.session.entropy
    else {
        unreachable!()
    };
    assert_eq!(
        (
            original_random.mask_calls.get(),
            original_random.rho_calls.get()
        ),
        (32768, 4)
    );
    assert_eq!(
        (*commitment_entropy_bytes, *q_mask_entropy_bytes),
        (128, 262144)
    );
    assert_eq!(controls::work_v1().selects, 4194304);
    let basis = Suite::generators().reduce(16384).unwrap();
    for digit in 0..4 {
        let rho = &stream.blindings.as_slice()[4 + digit];
        assert_eq!(*rho, Scalar::from_be_bytes_exact(rho_bytes(digit)).unwrap());
        let mut expected = SecretMultiexpBuilder::<Suite>::new(5).unwrap();
        for index in [0, 255, 256, 16383] {
            expected
                .push(
                    &Scalar::from_u64(coefficient(index) / 32768_u64.pow(digit as u32) % 32768),
                    &basis.g_bold[index],
                )
                .unwrap();
        }
        expected.push(rho, &basis.h).unwrap();
        assert_eq!(
            f.session.inventory.slots[27180 + digit]
                .as_ref()
                .unwrap()
                .point_wire,
            expected
                .evaluate()
                .unwrap()
                .expose_ref()
                .to_non_identity_wire_bytes()
                .unwrap()
        );
    }
    assert!(stream.finish_v1(&f.session, &f.table).is_err());
    drop(file);
    drop(f.table);
    assert_eq!(f.session.proof_resources.live_bytes().unwrap(), 0);
}
#[test]
#[cfg(unix)]
fn qmask_s_stream_next_admission_preserves_prefix_before_capacity_and_never_reseeds() {
    let directory = TestDirectory::new("qmask-s-stream-capacity");
    let mut probe = fixture(directory.path(), None);
    let stream = unqualified_stream_prefix(&mut probe);
    let admitted =
        QMaskSBlockAdmissionV1::next_v1(&mut probe.session, &probe.table, &stream, &probe.file)
            .unwrap();
    let exact = probe.session.proof_resources.live_bytes().unwrap();
    drop(admitted);
    drop(stream);
    drop(probe);
    let mut f = fixture(directory.path(), Some(exact));
    let stream = unqualified_stream_prefix(&mut f);
    let pointer = stream.block.coefficients.values.as_ptr();
    let rhos = stream.blindings.as_slice().as_ptr();
    let file_binding = f.file.require_block_binding_v1().unwrap();
    let before = f.session.proof_resources.live_bytes().unwrap();
    // The next-block entry rejects replay, skip and wrong-purpose state before
    // it allocates, samples or changes the actual closed file.
    for ordinal in [27176, 27179, 27181, 33576] {
        f.session.next_global_ordinal = ordinal;
        assert!(matches!(
            QMaskSBlockAdmissionV1::next_v1(&mut f.session, &f.table, &stream, &f.file),
            Err(QMaskSErrorV1::Source)
        ));
    }
    f.session.next_global_ordinal = 27180;
    f.session.next_purpose = GlobalLookupCommitmentPurposeV1::QMaskComplementDigit;
    assert!(matches!(
        QMaskSBlockAdmissionV1::next_v1(&mut f.session, &f.table, &stream, &f.file),
        Err(QMaskSErrorV1::Source)
    ));
    f.session.next_purpose = GlobalLookupCommitmentPurposeV1::QMaskDigit;
    f.session.inventory.slots[27183] = Some(GlobalLookupCommitmentTicketV1 {
        coordinate: commitment_coordinate_v1(27183).unwrap(),
        point_wire: Point::canonical_generator()
            .unwrap()
            .to_non_identity_wire_bytes()
            .unwrap(),
    });
    assert!(matches!(
        QMaskSBlockAdmissionV1::next_v1(&mut f.session, &f.table, &stream, &f.file),
        Err(QMaskSErrorV1::Source)
    ));
    f.session.inventory.slots[27183] = None;
    assert_eq!(f.session.proof_resources.live_bytes().unwrap(), before);
    let competitor = f
        .session
        .proof_resources
        .reserve_workspace_v1(1, 0)
        .unwrap();
    controls::reset_v1();
    assert!(matches!(
        QMaskSBlockAdmissionV1::next_v1(&mut f.session, &f.table, &stream, &f.file),
        Err(QMaskSErrorV1::Capacity)
    ));
    assert_eq!(controls::allocations_v1(), 0);
    assert_eq!(f.random().mask_calls.get(), 16384);
    assert_eq!(f.random().rho_calls.get(), 0);
    assert_eq!(stream.block.coefficients.values.as_ptr(), pointer);
    assert_eq!(stream.blindings.as_slice().as_ptr(), rhos);
    assert_eq!(f.file.require_block_binding_v1().unwrap(), file_binding);
    assert_eq!(f.file.block_ordinal_v1().unwrap(), 0);
    assert_eq!(f.session.proof_resources.live_bytes().unwrap(), before + 1);
    drop(competitor);
    let admission =
        QMaskSBlockAdmissionV1::next_v1(&mut f.session, &f.table, &stream, &f.file).unwrap();
    assert_eq!(f.session.proof_resources.live_bytes().unwrap(), exact);
    drop(admission);
    drop(stream);
    assert_eq!(f.session.next_global_ordinal, 27180);
}

#[test]
#[cfg(unix)]
fn qmask_s_stream_last_actual_block_seals_and_replays_same_file_rhos_and_zero_top() {
    let directory = TestDirectory::new("qmask-s-stream-last");
    let mut f = fixture(directory.path(), None);
    let mut stream = unqualified_stream_prefix(&mut f);
    // Explicit boundary fixture: previous6396 tickets are NOT genuine proofs.
    // The final four points, file seal, full read pass and original RNG are real.
    for i in 4..6396 {
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
    stream.block.coordinate = QMaskSBlockCoordinateV1::from_ordinal_v1(1598).unwrap();
    stream.block.coefficients.values.as_mut_slice().zeroize();
    f.session.next_global_ordinal = 33572;
    f.session.next_purpose_ordinal = 6396;
    let before_mask = stream.block.coordinate.sampled_values_through_v1() * 8;
    let GlobalLookupProofSessionEntropySourceV1::Production {
        original_random,
        commitment_entropy_bytes,
        q_mask_entropy_bytes,
    } = &mut f.session.entropy
    else {
        unreachable!()
    };
    *q_mask_entropy_bytes = before_mask;
    *commitment_entropy_bytes = 6396 * 32;
    original_random.mask_limit.set(32767);
    original_random.limb.set(39);
    let Fixture {
        mut session,
        mut table,
        file: mut written,
        storage,
        ..
    } = f;
    for block in 1..1599 {
        let mut writer = written.resume_next_block_v1().unwrap();
        for local in 0..8 {
            writer
                .write_slot_v1(
                    block * 8 + local,
                    ConfidentialSpoolChunkV1::new_zeroed_v1(16384).unwrap(),
                )
                .unwrap();
        }
        written = writer.finish_block_v1().unwrap();
    }
    assert_eq!(written.block_ordinal_v1().unwrap(), 1598);
    let pointer = stream.block.coefficients.values.as_ptr();
    let rhos = stream.blindings.as_slice().as_ptr();
    let admission =
        QMaskSBlockAdmissionV1::next_v1(&mut session, &table, &stream, &written).unwrap();
    let mut writer = written.resume_next_block_v1().unwrap();
    let stream = stream
        .continue_v1(&mut session, &mut table, &mut writer, admission)
        .unwrap();
    let written = writer.finish_block_v1().unwrap();
    assert_eq!(stream.block.coefficients.values.as_ptr(), pointer);
    assert_eq!(stream.blindings.as_slice().as_ptr(), rhos);
    assert_eq!(stream.blindings.len(), 6400);
    assert_eq!(stream.block.coefficients.values[16383], 0);
    assert_eq!(
        (
            session.next_global_ordinal,
            session.next_purpose,
            session.next_purpose_ordinal
        ),
        (
            33576,
            GlobalLookupCommitmentPurposeV1::QMaskComplementDigit,
            0
        )
    );
    let GlobalLookupProofSessionEntropySourceV1::Production {
        original_random,
        commitment_entropy_bytes,
        q_mask_entropy_bytes,
    } = &session.entropy
    else {
        unreachable!()
    };
    assert_eq!(
        (
            original_random.mask_calls.get(),
            original_random.rho_calls.get()
        ),
        (32767, 4)
    );
    assert_eq!(
        (*q_mask_entropy_bytes, *commitment_entropy_bytes),
        (before_mask + 16383 * 8, 6400 * 32)
    );
    let basis = Suite::generators().reduce(16384).unwrap();
    for digit in 0..4 {
        let mut expected = SecretMultiexpBuilder::<Suite>::new(4).unwrap();
        for index in [0, 255, 256] {
            let value = if index == 255 {
                ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[39] - 1
            } else {
                coefficient(index)
            };
            expected
                .push(
                    &Scalar::from_u64(value / 32768_u64.pow(digit as u32) % 32768),
                    &basis.g_bold[index],
                )
                .unwrap();
        }
        expected
            .push(&stream.blindings.as_slice()[6396 + digit], &basis.h)
            .unwrap();
        assert_eq!(
            session.inventory.slots[33572 + digit]
                .as_ref()
                .unwrap()
                .point_wire,
            expected
                .evaluate()
                .unwrap()
                .expose_ref()
                .to_non_identity_wire_bytes()
                .unwrap()
        );
    }
    let mut complete = stream.finish_v1(&session, &table).unwrap();
    assert!(!complete.loaded);
    assert!(
        complete
            .stream
            .block
            .coefficients
            .values
            .iter()
            .all(|x| *x == 0)
    );
    let mut file = written.seal_v1().unwrap();
    assert!(complete.require_replayed_v1(&file).is_err());
    for block in 0..1600 {
        complete
            .load_next_original_block_v1(&session, &mut file)
            .unwrap();
        assert!(complete.loaded);
        assert_eq!(complete.stream.block.coefficients.values.as_ptr(), pointer);
        assert_eq!(complete.stream.blindings.as_slice().as_ptr(), rhos);
        if block == 0 {
            assert_eq!(
                complete.stream.block.coefficients.values[255],
                ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0] - 1
            );
        } else if block == 1599 {
            assert_eq!(
                complete.stream.block.coefficients.values[255],
                ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[39] - 1
            );
            assert_eq!(complete.stream.block.coefficients.values[16383], 0);
        } else {
            assert!(
                complete
                    .stream
                    .block
                    .coefficients
                    .values
                    .iter()
                    .all(|x| *x == 0)
            );
        }
        complete.erase_loaded_block_v1().unwrap();
        assert!(
            complete
                .stream
                .block
                .coefficients
                .values
                .iter()
                .all(|x| *x == 0)
        );
    }
    complete.require_replayed_v1(&file).unwrap();
    assert!(
        complete
            .load_next_original_block_v1(&session, &mut file)
            .is_err()
    );
    drop(complete);
    drop(file);
    drop(table);
    assert_eq!(session.proof_resources.live_bytes().unwrap(), 0);
    assert_eq!(storage.test_usage_words_v1()[0], 0);
}

#[test]
#[cfg(unix)]
fn qmask_s_stream_continuation_entropy_failure_consumes_refilled_preimage_after_actual_writes() {
    let directory = TestDirectory::new("qmask-s-stream-entropy-failure");
    let mut f = fixture(directory.path(), None);
    let stream = unqualified_stream_prefix(&mut f);
    f.random().mask_limit.set(32768);
    f.random().fault.set(Fault::Zero);
    let admission =
        QMaskSBlockAdmissionV1::next_v1(&mut f.session, &f.table, &stream, &f.file).unwrap();
    let mut writer = f.file.resume_next_block_v1().unwrap();
    let before = ZEROIZED_MASK_VALUES_V1.with(Cell::get);
    assert!(matches!(
        stream.continue_v1(&mut f.session, &mut f.table, &mut writer, admission),
        Err(QMaskSErrorV1::Entropy)
    ));
    assert_eq!(ZEROIZED_MASK_VALUES_V1.with(Cell::get), before + 16384);
    let GlobalLookupProofSessionEntropySourceV1::Production {
        original_random,
        commitment_entropy_bytes,
        q_mask_entropy_bytes,
    } = &f.session.entropy
    else {
        unreachable!()
    };
    assert_eq!(
        (
            original_random.mask_calls.get(),
            original_random.rho_calls.get()
        ),
        (32768, 128)
    );
    assert_eq!(
        (*q_mask_entropy_bytes, *commitment_entropy_bytes),
        (262144, 4096)
    );
    assert_eq!(f.session.next_global_ordinal, 27180);
    assert!(
        f.session.inventory.slots[27180..]
            .iter()
            .all(Option::is_none)
    );
    // This lower fixture can still see the actual written file. The consuming
    // original outer transition returns no owner on this post-entropy failure.
    assert_eq!(
        writer
            .finish_block_v1()
            .unwrap()
            .block_ordinal_v1()
            .unwrap(),
        1
    );
    drop(f.table);
    assert_eq!(f.session.proof_resources.live_bytes().unwrap(), 0);
    assert_eq!(f.storage.test_usage_words_v1()[0], 0);
}

#[cfg(unix)]
pub(in super::super::super) fn with_s_stream_for_retained_refusal_v1(
    check: impl FnOnce(QMaskSOpeningStreamV1, WrittenQMaskSBlockFileV1, QMaskSBlockAdmissionV1),
) {
    let directory = TestDirectory::new("qmask-s-stream-retained-refusal");
    let mut f = fixture(directory.path(), None);
    let stream = unqualified_stream_prefix(&mut f);
    let admission =
        QMaskSBlockAdmissionV1::next_v1(&mut f.session, &f.table, &stream, &f.file).unwrap();
    let rho_calls = Rc::clone(&f.random().rho_calls);
    let mask_calls = Rc::clone(&f.random().mask_calls);
    let before = ZEROIZED_MASK_VALUES_V1.with(Cell::get);
    check(stream, f.file, admission);
    assert_eq!(rho_calls.get(), 0);
    assert_eq!(mask_calls.get(), 16384);
    assert_eq!(ZEROIZED_MASK_VALUES_V1.with(Cell::get), before + 16384);
}

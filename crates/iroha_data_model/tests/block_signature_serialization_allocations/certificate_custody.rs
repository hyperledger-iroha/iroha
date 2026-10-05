//! Genuine original byte backing, physical refusal and immutable last-reader custody.
use super::*;
use iroha_allocation::{
    AllocationBudget, ChargedBuffer, ChargedBufferFromChargeError, ChargedShared,
    SharedFromChargeError,
};
use iroha_crypto::Hash;
use iroha_data_model::block::commit_certificate::{
    CertificateCustodyError, PreparedCommitCertificate,
};
use iroha_data_model::block::{ChargedCertificateParts, CommitCertificate};
use norito::core::SequenceSpan;

fn fixture() -> CommitCertificate {
    CommitCertificate::from_untrusted_parts(vec![11; 11], vec![17; 17], vec![23; 23], vec![31; 31])
}
fn source(
    original: &CommitCertificate,
    budget: &AllocationBudget,
) -> (ChargedBuffer<u8>, SequenceSpan) {
    let wire = bare_bytes(original, header_flags::COMPACT_LEN);
    let mut source = ChargedBuffer::new(wire.len() + 9, budget).unwrap();
    source.append(&[0xad; 9]).unwrap();
    source.append(&wire).unwrap();
    let span = SequenceSpan {
        start: 9,
        end: 9 + wire.len(),
    };
    (source, span)
}
fn measured<T>(run: impl FnOnce() -> T) -> (T, usize) {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            TRACKING.with(|active| active.set(false));
        }
    }
    TRACKING.with(|active| assert!(!active.replace(true)));
    ALLOCATIONS.with(|count| count.set(0));
    let guard = Restore;
    let value = run();
    let count = ALLOCATIONS.with(Cell::get);
    drop(guard);
    (value, count)
}
fn refused<T>(layout: Layout, run: impl FnOnce() -> T) -> T {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            REFUSE.with(|next| next.set(None));
        }
    }
    REFUSE.with(|next| assert!(next.replace(Some(layout)).is_none()));
    let guard = Restore;
    let value = run();
    assert!(
        REFUSE.with(|next| next.get().is_none()),
        "exact original allocator request was not reached"
    );
    drop(guard);
    value
}
#[test]
fn certificate_four_children_and_control_are_physically_admitted_before_fill() {
    let _flags = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
    let original = fixture();
    let budget = AllocationBudget::new(1024 * 1024);
    let (source, span) = source(&original, &budget);
    let floor = budget.reserved_bytes();
    let (pending, count) =
        measured(|| PreparedCommitCertificate::from_source(&source, span, &budget));
    assert_eq!(
        count, 0,
        "canonical borrowed planning must not allocate owning children"
    );
    let mut pending = pending.unwrap();
    let layouts = pending.allocation_layouts().unwrap();
    assert_eq!(
        layouts[..4].iter().map(Layout::size).collect::<Vec<_>>(),
        [11, 17, 23, 31]
    );
    assert_eq!(
        layouts[4],
        ChargedShared::<ChargedCertificateParts>::allocation_layout()
    );
    let required = layouts.iter().map(Layout::size).sum::<usize>();
    budget.set_limit_bytes(floor + required - 1);
    let (error, count) = measured(|| pending.prepare(&source));
    assert_eq!(count, 0);
    assert!(matches!(error, Err(CertificateCustodyError::Admission(_))));
    assert_eq!(pending.initialized(&source).unwrap(), [None; 4]);
    assert_eq!(budget.reserved_bytes(), floor);
    budget.set_limit_bytes(floor + required);
    let (filled, count) = measured(|| pending.prepare(&source));
    filled.unwrap();
    assert_eq!(
        count, 5,
        "exact four nonzero backings plus original shared shell"
    );
    assert_eq!(budget.reserved_bytes(), floor + required);
    assert!(pending.belongs_to(&budget));
    let (same, count) = measured(|| pending.prepare(&source));
    same.unwrap();
    assert_eq!(count, 0);
    let (admitted, count) = measured(|| pending.finish(&source));
    let admitted = admitted.unwrap_or_else(|_| panic!("complete original owner"));
    assert_eq!(count, 0);
    assert!(admitted.admitted_to(&budget));
    assert_eq!(admitted, original);
    assert_eq!(
        norito::to_bytes(&admitted).unwrap(),
        norito::to_bytes(&original).unwrap()
    );
    assert_eq!(
        Hash::new(norito::to_bytes(&admitted).unwrap()),
        Hash::new(norito::to_bytes(&original).unwrap())
    );
    assert_eq!(
        bare_bytes(&admitted, header_flags::COMPACT_LEN),
        span.get(source.as_slice()).unwrap()
    );
    let (reader, count) = measured(|| admitted.clone());
    assert_eq!(count, 0);
    assert!(CommitCertificate::ptr_eq(&admitted, &reader));
    let pointers = [
        reader.consensus_header().as_ptr(),
        reader.commit_qc().as_ptr(),
        reader.result_preimage().as_ptr(),
        reader.availability().as_ptr(),
    ];
    drop(admitted);
    assert_eq!(budget.reserved_bytes(), floor + required);
    assert_eq!(
        [
            reader.consensus_header().as_ptr(),
            reader.commit_qc().as_ptr(),
            reader.result_preimage().as_ptr(),
            reader.availability().as_ptr()
        ],
        pointers
    );
    drop(reader);
    assert_eq!(budget.reserved_bytes(), floor);
    drop(source);
    assert_eq!(budget.reserved_bytes(), 0);
}
#[test]
fn certificate_each_allocator_refusal_keeps_original_siblings_and_all_prepaid_credit() {
    let _flags = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
    for selected in 0..5 {
        let original = fixture();
        let budget = AllocationBudget::new(1024 * 1024);
        let (source, span) = source(&original, &budget);
        let source_pointer = source.as_slice().as_ptr();
        let source_hash = Hash::new(source.as_slice());
        let floor = budget.reserved_bytes();
        let mut pending = PreparedCommitCertificate::from_source(&source, span, &budget).unwrap();
        let layouts = pending.allocation_layouts().unwrap();
        let required = layouts.iter().map(Layout::size).sum::<usize>();
        let error = refused(layouts[selected], || pending.prepare(&source)).unwrap_err();
        if selected < 4 {
            assert!(
                matches!(error, CertificateCustodyError::Buffer(ChargedBufferFromChargeError::Allocator { layout }) if layout == layouts[selected])
            );
        } else {
            assert!(
                matches!(error, CertificateCustodyError::Control(SharedFromChargeError::Allocator { layout }) if layout == layouts[selected])
            );
        }
        assert_eq!(
            budget.reserved_bytes(),
            floor + required,
            "failed actual allocation retains its exact charge"
        );
        assert!(pending.belongs_to(&budget));
        let pointers = pending
            .initialized(&source)
            .unwrap()
            .map(|leaf| leaf.map(<[u8]>::as_ptr));
        for (index, pointer) in pointers.into_iter().enumerate() {
            assert_eq!(pointer.is_some(), index < selected.min(4));
            assert_eq!(
                pending.initialized(&source).unwrap()[index].map(<[u8]>::len),
                if index < selected.min(4) {
                    Some(0)
                } else {
                    None
                },
                "nothing is filled until all five actual allocations exist"
            );
        }
        budget.set_limit_bytes(0);
        pending.prepare(&source).unwrap();
        assert_eq!(budget.reserved_bytes(), floor + required);
        for (before, after) in pointers
            .into_iter()
            .zip(pending.initialized(&source).unwrap())
        {
            if let Some(before) = before {
                assert_eq!(before, after.unwrap().as_ptr());
            }
        }
        assert_eq!(source.as_slice().as_ptr(), source_pointer);
        assert_eq!(Hash::new(source.as_slice()), source_hash);
        let owner = pending
            .finish(&source)
            .unwrap_or_else(|_| panic!("same original refusal retry"));
        assert_eq!(owner, original);
        drop(owner);
        assert_eq!(budget.reserved_bytes(), floor);
        drop(source);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
#[test]
fn certificate_partial_and_published_unwind_retire_only_the_original_physical_owners() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let _flags = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
    for published in [false, true] {
        let original = fixture();
        let budget = AllocationBudget::new(1024 * 1024);
        let (source, span) = source(&original, &budget);
        let floor = budget.reserved_bytes();
        let failed = catch_unwind(AssertUnwindSafe(|| {
            let mut pending =
                PreparedCommitCertificate::from_source(&source, span, &budget).unwrap();
            if published {
                pending.prepare(&source).unwrap();
                let owner = pending
                    .finish(&source)
                    .unwrap_or_else(|_| panic!("complete original owner"));
                let reader = owner.clone();
                assert!(CommitCertificate::ptr_eq(&owner, &reader));
                assert!(reader.admitted_to(&budget));
                panic!("interrupted with original published readers");
            } else {
                let layout = pending.allocation_layouts().unwrap()[2];
                assert!(refused(layout, || pending.prepare(&source)).is_err());
                assert!(pending.initialized(&source).unwrap()[0].is_some());
                panic!("interrupted with original partial decoder");
            }
        }));
        assert!(failed.is_err());
        assert_eq!(budget.reserved_bytes(), floor);
        assert_eq!(
            span.get(source.as_slice()).unwrap(),
            bare_bytes(&original, header_flags::COMPACT_LEN)
        );
        drop(source);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn certificate_completion_rejects_unprepared_foreign_changed_source_and_layout_without_allocation()
{
    let _flags = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
    let original = fixture();
    let budget = AllocationBudget::new(1024 * 1024);
    let foreign = AllocationBudget::new(1024 * 1024);
    let (mut input, span) = source(&original, &budget);
    let (equal_copy, _) = source(&original, &budget);
    let (foreign_source, foreign_span) = source(&original, &foreign);
    assert!(matches!(
        PreparedCommitCertificate::from_source(&foreign_source, foreign_span, &budget),
        Err(CertificateCustodyError::ForeignPool)
    ));
    let pending = PreparedCommitCertificate::from_source(&input, span, &budget).unwrap();
    let (mut pending, error) = pending.finish(&input).err().unwrap();
    assert!(matches!(error, CertificateCustodyError::Incomplete));
    assert!(pending.belongs_to(&budget));
    let floor = budget.reserved_bytes();
    let (error, count) = measured(|| pending.prepare(&equal_copy));
    assert!(matches!(error, Err(CertificateCustodyError::SourceChanged)));
    assert_eq!(count, 0);
    assert!(matches!(
        pending.prepare(&foreign_source),
        Err(CertificateCustodyError::ForeignPool)
    ));
    let old = input.as_slice()[0];
    input.as_mut_slice()[0] ^= 1;
    let (error, count) = measured(|| pending.prepare(&input));
    assert!(matches!(error, Err(CertificateCustodyError::SourceChanged)));
    assert_eq!(count, 0);
    input.as_mut_slice()[0] = old;
    {
        let _different = DecodeFlagsGuard::enter(0);
        let (error, count) = measured(|| pending.prepare(&input));
        assert!(matches!(error, Err(CertificateCustodyError::LayoutChanged)));
        assert_eq!(count, 0);
    }
    assert_eq!(budget.reserved_bytes(), floor);
    pending.prepare(&input).unwrap();
    let owner = pending
        .finish(&input)
        .unwrap_or_else(|_| panic!("exact restored original source"));
    assert_eq!(owner, original);
    drop(owner);
    assert_eq!(budget.reserved_bytes(), floor);
    drop(input);
    drop(equal_copy);
    drop(foreign_source);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(foreign.reserved_bytes(), 0);
}

#[test]
fn certificate_zero_length_children_keep_exact_original_control_and_last_reader() {
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        for lengths in [[0, 0, 0, 0], [0, 17, 0, 31], [11, 0, 23, 0]] {
            let original = CommitCertificate::from_untrusted_parts(
                vec![11; lengths[0]],
                vec![17; lengths[1]],
                vec![23; lengths[2]],
                vec![31; lengths[3]],
            );
            let wire = bare_bytes(&original, flags);
            let budget = AllocationBudget::new(1024 * 1024);
            let mut source = ChargedBuffer::new(wire.len() + 9, &budget).unwrap();
            source.append(&[0xad; 9]).unwrap();
            source.append(&wire).unwrap();
            let span = SequenceSpan {
                start: 9,
                end: 9 + wire.len(),
            };
            let floor = budget.reserved_bytes();
            let pointer = source.as_slice().as_ptr();
            let hash = Hash::new(source.as_slice());
            let (pending, count) =
                measured(|| PreparedCommitCertificate::from_source(&source, span, &budget));
            assert_eq!(count, 0);
            let mut pending = pending.unwrap();
            let layouts = pending.allocation_layouts().unwrap();
            for (layout, length) in layouts[..4].iter().zip(lengths) {
                assert_eq!(*layout, Layout::array::<u8>(length).unwrap());
            }
            assert_eq!(
                layouts[4],
                ChargedShared::<ChargedCertificateParts>::allocation_layout()
            );
            let required = layouts.iter().map(Layout::size).sum::<usize>();
            let nonempty = lengths.iter().filter(|length| **length != 0).count();
            budget.set_limit_bytes(floor + required);
            let (error, count) = measured(|| refused(layouts[4], || pending.prepare(&source)));
            assert!(
                matches!(error, Err(CertificateCustodyError::Control(SharedFromChargeError::Allocator { layout })) if layout == layouts[4])
            );
            assert_eq!(
                count,
                nonempty + 1,
                "zero-layout children request no allocator storage; only nonempty leaves and the original control are requested"
            );
            assert_eq!(budget.reserved_bytes(), floor + required);
            assert!(pending.belongs_to(&budget));
            let prefixes = pending.initialized(&source).unwrap();
            assert!(
                prefixes
                    .iter()
                    .all(|leaf| leaf.is_some_and(<[u8]>::is_empty)),
                "every original zero-layout or nonempty backing exists before any fill"
            );
            let pointers = prefixes.map(|leaf| leaf.unwrap().as_ptr());
            budget.set_limit_bytes(0);
            let (filled, count) = measured(|| pending.prepare(&source));
            filled.unwrap();
            assert_eq!(
                count, 1,
                "only the refused original shared control is allocated; canonical byte fill adds no allocation"
            );
            assert_eq!(budget.reserved_bytes(), floor + required);
            assert!(pending.belongs_to(&budget));
            for ((leaf, length), pointer) in pending
                .initialized(&source)
                .unwrap()
                .into_iter()
                .zip(lengths)
                .zip(pointers)
            {
                let leaf = leaf.unwrap();
                assert_eq!(leaf.len(), length);
                assert_eq!(leaf.as_ptr(), pointer);
            }
            let (retried, count) = measured(|| pending.prepare(&source));
            retried.unwrap();
            assert_eq!(count, 0);
            let (admitted, count) = measured(|| pending.finish(&source));
            let admitted =
                admitted.unwrap_or_else(|_| panic!("same original complete certificate"));
            assert_eq!(count, 0);
            assert!(admitted.admitted_to(&budget));
            assert_eq!(admitted, original);
            assert_eq!(bare_bytes(&admitted, flags), wire);
            assert_eq!(
                norito::to_bytes(&admitted).unwrap(),
                norito::to_bytes(&original).unwrap()
            );
            assert_eq!(
                Hash::new(norito::to_bytes(&admitted).unwrap()),
                Hash::new(norito::to_bytes(&original).unwrap())
            );
            assert_eq!(source.as_slice().as_ptr(), pointer);
            assert_eq!(Hash::new(source.as_slice()), hash);
            let (reader, count) = measured(|| admitted.clone());
            assert_eq!(count, 0);
            assert!(CommitCertificate::ptr_eq(&admitted, &reader));
            drop(admitted);
            assert_eq!(budget.reserved_bytes(), floor + required);
            assert!(reader.admitted_to(&budget));
            assert_eq!(
                [
                    reader.consensus_header().as_ptr(),
                    reader.commit_qc().as_ptr(),
                    reader.result_preimage().as_ptr(),
                    reader.availability().as_ptr()
                ],
                pointers
            );
            drop(source);
            assert_eq!(
                budget.reserved_bytes(),
                required,
                "the published original control owns all four leaves independently of the source"
            );
            assert_eq!(reader, original);
            drop(reader);
            assert_eq!(budget.reserved_bytes(), 0);
        }
    }
}

//! Actual allocation, source retention and destruction order for signature custody.

use iroha_crypto::{Signature, SignatureAllocationError, verify_signature_borrowed};

use super::*;

#[test]
fn exact_signature_refusal_preserves_source_and_original_prepaid_charge_for_retry() {
    let _serial = SERIAL.lock().unwrap();
    let pair = key();
    let message = b"original prepaid signature custody";
    let source = Signature::new(pair.private_key(), message);
    let source_pointer = source.payload().as_ptr();
    let layout = source.retained_allocation_layout();
    let pool = AllocationBudget::new(layout.size());
    let original = charge(&pool, layout);
    arm(&pool, true);
    let result = source.try_clone_from_charge(&pool, original);
    disarm();
    let (original, error) = match result {
        Err(error) => error,
        Ok(_) => panic!("the physical allocator was instructed to refuse"),
    };
    assert!(matches!(error, SignatureAllocationError::Allocation(
        ChargedBufferFromChargeError::Allocator { layout: refused }) if refused == layout));
    assert_eq!(CALLS.load(SeqCst), 1);
    assert_eq!(original.layout(), layout);
    assert!(original.belongs_to(&pool));
    assert_eq!(pool.reserved_bytes(), layout.size());
    assert_eq!(source.payload().as_ptr(), source_pointer);
    // Retry owns already-admitted credit even after the policy limit is reduced.
    pool.set_limit_bytes(0);
    arm(&pool, false);
    let copied = source.try_clone_from_charge(&pool, original).unwrap();
    disarm();
    assert_eq!(CALLS.load(SeqCst), 1);
    assert_eq!(copied.get(), &source);
    assert_ne!(copied.get().payload().as_ptr(), source_pointer);
    assert_eq!(RESERVED_AT_ALLOC.load(SeqCst), layout.size());
    verify_signature_borrowed(copied.get(), pair.public_key(), message).unwrap();
    assert!(verify_signature_borrowed(copied.get(), pair.public_key(), b"changed").is_err());
    drop(copied);
    assert_eq!(FREES.load(SeqCst), 1);
    assert_eq!(RESERVED_AT_FREE.load(SeqCst), layout.size());
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(source.payload().as_ptr(), source_pointer);
}

#[test]
fn signature_foreign_source_and_exact_layout_fail_before_allocation_or_refund() {
    let _serial = SERIAL.lock().unwrap();
    let source = Signature::from_bytes(&[0x57; 64]);
    let layout = source.retained_allocation_layout();
    let pool = AllocationBudget::new(4096);
    let foreign = AllocationBudget::new(4096);
    let original = charge(&foreign, layout);
    arm(&pool, true);
    let result = source.try_clone_from_charge(&pool, original);
    disarm();
    let (original, error) = match result {
        Err(error) => error,
        Ok(_) => panic!("equal finite limits cannot replace original pool identity"),
    };
    assert_eq!(error, SignatureAllocationError::ForeignPool);
    assert_eq!(CALLS.load(SeqCst), 0);
    assert!(original.belongs_to(&foreign));
    assert_eq!(original.layout(), layout);
    assert_eq!(foreign.reserved_bytes(), layout.size());
    assert_eq!(pool.reserved_bytes(), 0);
    drop(original);
    assert_eq!(foreign.reserved_bytes(), 0);
    for wrong in [
        Layout::from_size_align(layout.size() + 1, 1).unwrap(),
        Layout::from_size_align(layout.size(), 2).unwrap(),
    ] {
        let original = charge(&pool, wrong);
        arm(&pool, true);
        let result = source.try_clone_from_charge(&pool, original);
        disarm();
        let (original, error) = match result {
            Err(error) => error,
            Ok(_) => panic!("size and alignment must both match the canonical payload"),
        };
        assert!(matches!(error, SignatureAllocationError::Allocation(
            ChargedBufferFromChargeError::LayoutMismatch { expected, actual })
            if expected == layout && actual == wrong));
        assert_eq!(CALLS.load(SeqCst), 0);
        assert!(original.belongs_to(&pool));
        assert_eq!(original.layout(), wrong);
        assert_eq!(pool.reserved_bytes(), wrong.size());
        drop(original);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn signature_copy_preserves_empty_and_malformed_geometry_without_authorizing_it() {
    let _serial = SERIAL.lock().unwrap();
    let pair = key();
    for bytes in [&[][..], &[0_u8; 64][..], &[0x81_u8; 3][..]] {
        let source = Signature::from_bytes(bytes);
        let source_pointer = source.payload().as_ptr();
        let layout = source.retained_allocation_layout();
        assert_eq!(layout, Layout::array::<u8>(bytes.len()).unwrap());
        let pool = AllocationBudget::new(layout.size());
        let original = charge(&pool, layout);
        arm(&pool, false);
        let copied = source.try_clone_from_charge(&pool, original).unwrap();
        disarm();
        assert_eq!(CALLS.load(SeqCst), usize::from(!bytes.is_empty()));
        assert_eq!(copied.get().payload(), bytes);
        assert_eq!(source.payload().as_ptr(), source_pointer);
        if !bytes.is_empty() {
            assert_ne!(copied.get().payload().as_ptr(), source_pointer);
        }
        assert!(verify_signature_borrowed(copied.get(), pair.public_key(), b"message").is_err());
        assert_eq!(pool.reserved_bytes(), layout.size());
        drop(copied);
        assert_eq!(FREES.load(SeqCst), usize::from(!bytes.is_empty()));
        assert_eq!(RESERVED_AT_FREE.load(SeqCst), layout.size());
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
#[allow(unsafe_code)]
fn original_signature_allocation_survives_audited_canonical_moves_until_final_drop() {
    struct Canonical {
        signature: Signature,
    }
    let _serial = SERIAL.lock().unwrap();
    let pair = key();
    let message = b"audited signature ledger transfer";
    let source = Signature::new(pair.private_key(), message);
    let layout = source.retained_allocation_layout();
    let ledger_layout = Layout::array::<AllocationCharge>(1).unwrap();
    let pool = AllocationBudget::new(layout.size() + ledger_layout.size());
    let mut ledger = ChargedBuffer::new(1, &pool).unwrap();
    let original = charge(&pool, layout);
    arm(&pool, false);
    let copied = source.try_clone_from_charge(&pool, original).unwrap();
    disarm();
    let pointer = copied.get().payload().as_ptr();
    // SAFETY: move the same payload and charge directly into the private
    // canonical record and original ledger, with no intervening failure.
    let (signature, original) = unsafe { copied.into_allocation_parts() };
    ledger.push_reserved(original);
    let payload = unsafe { RetainedPayload::try_new(Canonical { signature }, ledger, &pool) }
        .unwrap_or_else(|refusal| panic!("same original pool ledger: {}", refusal.2));
    // SAFETY: move the canonical field unchanged into an inline envelope.
    // No signature allocation is replaced, shared, cloned or destroyed here.
    let published = unsafe { payload.map_payload(|canonical| (canonical, 11_u64)) };
    assert_eq!(CALLS.load(SeqCst), 1);
    assert_eq!(published.get().0.signature.payload().as_ptr(), pointer);
    assert_eq!(&published.get().0.signature, &source);
    verify_signature_borrowed(&published.get().0.signature, pair.public_key(), message).unwrap();
    assert_eq!(pool.reserved_bytes(), layout.size() + ledger_layout.size());
    drop(published);
    assert_eq!(FREES.load(SeqCst), 1);
    assert_eq!(
        RESERVED_AT_FREE.load(SeqCst),
        layout.size() + ledger_layout.size()
    );
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn unwinding_signature_owner_destroys_payload_before_refunding_original_credit() {
    let _serial = SERIAL.lock().unwrap();
    let source = Signature::from_bytes(&[0x63; 96]);
    let source_pointer = source.payload().as_ptr();
    let layout = source.retained_allocation_layout();
    let pool = AllocationBudget::new(layout.size());
    let original = charge(&pool, layout);
    arm(&pool, false);
    let copied = source.try_clone_from_charge(&pool, original).unwrap();
    disarm();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _retained = copied;
        panic!("exercise canonical signature owner cleanup");
    }));
    assert!(result.is_err());
    assert_eq!(CALLS.load(SeqCst), 1);
    assert_eq!(FREES.load(SeqCst), 1);
    assert_eq!(RESERVED_AT_FREE.load(SeqCst), layout.size());
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(source.payload().as_ptr(), source_pointer);
}

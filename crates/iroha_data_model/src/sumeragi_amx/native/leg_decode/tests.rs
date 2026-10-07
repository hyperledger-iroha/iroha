//! Canonical graph parity, actual nested layouts and original-prefix retry controls.

use super::*;
use iroha_allocation::{AllocationRefusal, release::ReleaseRegistration};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_model_base::domain::DomainId;
use norito::core::{DecodeBudgetContext, DecodeLimits, SequenceSpan};
use std::{
    alloc::Layout,
    task::{Context, Poll, Waker},
};

fn key(seed: u8) -> PublicKey {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
        .public_key()
        .clone()
}
fn single(seed: u8) -> AccountId {
    AccountId::new(key(seed))
}
fn multisig(seed: u8) -> AccountId {
    AccountId::new_multisig(
        MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(key(seed), 1).unwrap(),
                MultisigMember::new(key(seed + 1), 2).unwrap(),
            ],
        )
        .unwrap(),
    )
}
fn leg(source: AccountId, destination: AccountId, amount: Quantity) -> AmxTransferLegV1 {
    AmxTransferLegV1 {
        source: AssetId::with_scope(
            AssetDefinitionId::derive_from_components(
                DomainId::try_new("native", "amx").unwrap(),
                "currency".parse().unwrap(),
            ),
            source,
            AssetBalanceScope::Dataspace(DataSpaceId::new(u64::MAX)),
        ),
        destination,
        amount,
    }
}
fn actual_controller_layouts(account: &AccountId) -> usize {
    match account.controller() {
        AccountController::Single(key) => key.retained_allocation_layout().size(),
        AccountController::Multisig(policy) => {
            let count = policy.members().len();
            Layout::array::<MultisigMember>(count).unwrap().size()
                + Layout::array::<AllocationCharge>(count).unwrap().size()
                + policy
                    .members()
                    .iter()
                    .map(|member| member.public_key().retained_allocation_layout().size())
                    .sum::<usize>()
        }
    }
}
fn exact_retained(leg: &AmxTransferLegV1) -> usize {
    actual_controller_layouts(leg.source.account())
        + actual_controller_layouts(&leg.destination)
        + leg.amount.admission_clone_layout().unwrap().size()
}
fn controls() -> usize {
    PreparedDecodeWorkspace::allocation_layouts()
        .iter()
        .map(Layout::size)
        .sum()
}
fn registration() -> ReleaseRegistration {
    let layout = ReleaseRegistration::allocation_layout();
    let pool = AllocationBudget::new(layout.size());
    let mut original = pool.try_reserve(layout).unwrap();
    ReleaseRegistration::from_reservation(&mut original).unwrap()
}

#[test]
fn funded_native_leg_matches_ordinary_canonical_single_multisig_uuid_scope_and_digits() {
    for (source, destination) in [
        (single(1), single(3)),
        (multisig(1), single(5)),
        (single(1), multisig(5)),
        (multisig(1), multisig(5)),
    ] {
        for amount in [
            Quantity::from(0_u32),
            Quantity::from(17_u32),
            Quantity::from(u128::MAX),
        ] {
            let original = leg(source.clone(), destination.clone(), amount);
            let bytes = norito::encode_canonical(&original).unwrap();
            let ordinary = norito::decode_canonical::<AmxTransferLegV1>(&bytes).unwrap();
            let expected = exact_retained(&ordinary);
            // Span scratch is real transient storage and must coexist with the
            // partially built controller. It is absent from the final graph.
            let spans = Layout::array::<SequenceSpan>(2).unwrap().size();
            let pool = AllocationBudget::new(expected + spans + controls());
            let foreign = AllocationBudget::new(expected + spans + controls());
            let pending = PendingAmxTransferLegDecodeV1::new(&bytes, &pool);
            assert!(core::ptr::eq(
                pending.original_source().as_ptr(),
                bytes.as_ptr()
            ));
            let decoded = pending.try_decode().unwrap();
            assert_eq!(decoded.canonical(), &ordinary);
            assert_eq!(
                norito::encode_canonical(decoded.canonical()).unwrap(),
                bytes
            );
            assert!(decoded.belongs_to(&pool));
            assert!(!decoded.belongs_to(&foreign));
            assert_eq!(decoded.allocation_bytes(), Some(expected));
            assert_eq!(pool.reserved_bytes(), expected);
            assert_eq!(
                decoded
                    .canonical()
                    .amount
                    .mantissa()
                    .admission_clone_layout()
                    .unwrap(),
                ordinary.amount.mantissa().admission_clone_layout().unwrap()
            );
            let key_sum = |account: &AccountId| actual_controller_layouts(account);
            assert_eq!(
                key_sum(decoded.canonical().source.account()),
                key_sum(ordinary.source.account())
            );
            assert_eq!(
                key_sum(&decoded.canonical().destination),
                key_sum(&ordinary.destination)
            );
            pool.set_limit_bytes(0);
            drop(decoded);
            assert_eq!(pool.reserved_bytes(), 0);
        }
    }
}

#[test]
fn funded_native_leg_preserves_exact_original_capacity_release_generation_and_final_refund() {
    let original = leg(single(1), single(3), Quantity::from(17_u32));
    let bytes = norito::encode_canonical(&original).unwrap();
    let first = PreparedDecodeWorkspace::allocation_layouts();
    let control_bytes = controls();
    let pool = AllocationBudget::new(control_bytes);
    let foreign = AllocationBudget::new(control_bytes);
    let blocker = ChargedBuffer::<u8>::new(control_bytes, &pool).unwrap();
    let foreign_blocker = ChargedBuffer::<u8>::new(control_bytes, &foreign).unwrap();
    let expected = pool.try_reserve_layouts(first).unwrap_err();
    let unrelated = foreign.try_reserve_layouts(first).unwrap_err();
    let pending = PendingAmxTransferLegDecodeV1::new(&bytes, &pool);
    let source = (
        pending.original_source().as_ptr(),
        pending.original_source().len(),
    );
    let failure = pending.try_decode().unwrap_err();
    let Error::Allocation(ChargedBufferError::Admission(actual)) = failure else {
        panic!("original Capacity was erased")
    };
    assert_eq!(
        actual, expected,
        "includes the exact original pool and observed release generation"
    );
    assert_ne!(actual, unrelated);
    let AllocationRefusal::Capacity { release, .. } = &actual else {
        panic!("actual occupied pool")
    };
    let mut waiter = registration();
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(waiter.poll_wait(release, &mut context), Poll::Pending);
    drop(foreign_blocker);
    assert_eq!(waiter.poll_wait(release, &mut context), Poll::Pending);
    assert_eq!(
        (
            pending.original_source().as_ptr(),
            pending.original_source().len()
        ),
        source
    );
    assert_eq!(pool.reserved_bytes(), control_bytes);
    drop(blocker);
    assert_eq!(waiter.poll_wait(release, &mut context), Poll::Ready(()));
    assert_eq!(pool.reserved_bytes(), 0);
    let expected = exact_retained(&original);
    pool.set_limit_bytes(expected + controls());
    let owner = pending.try_decode().unwrap();
    assert_eq!(owner.canonical(), &original);
    assert_eq!(pool.reserved_bytes(), expected);
    pool.set_limit_bytes(expected);
    let probe = key(1).retained_allocation_layout();
    let full = pool.try_reserve(probe).unwrap_err();
    let AllocationRefusal::Capacity { release, .. } = &full else {
        panic!("retained graph must occupy exact credit")
    };
    assert_eq!(waiter.poll_wait(release, &mut context), Poll::Pending);
    let same_generation = pool.try_reserve(probe).unwrap_err();
    assert_eq!(same_generation, full);
    pool.set_limit_bytes(0);
    drop(owner);
    assert_eq!(waiter.poll_wait(release, &mut context), Poll::Ready(()));
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(
        (
            pending.original_source().as_ptr(),
            pending.original_source().len()
        ),
        source
    );
}

#[test]
fn funded_multisig_members_keep_actual_vector_capacity_key_ledger_and_existing_relation() {
    let account = multisig(1);
    let policy = account.multisig_policy().unwrap();
    let _flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    let mut body = Vec::new();
    ncore::serialize_to_buffer(policy, &mut body).unwrap();
    let _context = ncore::PayloadCtxGuard::enter(&body);
    let mut offset = 0;
    let _version = ncore::framed_field::<u8>(&body, &mut offset).unwrap();
    let _threshold = ncore::framed_field::<u16>(&body, &mut offset).unwrap();
    let field = ncore::framed_field::<Vec<MultisigMember>>(&body, &mut offset).unwrap();
    let pool = AllocationBudget::new(4096);
    let parts = field
        .with_payload(|bytes| members(bytes, &pool).map_err(destination))
        .unwrap();
    let count = policy.members().len();
    let values = parts.members.as_ref().unwrap();
    assert_eq!(values.capacity(), count);
    assert_eq!(values.len(), count);
    let original = values.as_ptr();
    assert_eq!(
        parts.charge.layout(),
        Layout::array::<MultisigMember>(values.capacity()).unwrap()
    );
    assert_eq!(parts.keys.capacity(), count);
    assert_eq!(parts.keys.as_slice().len(), count);
    for (member, charge) in values.iter().zip(parts.keys.as_slice()) {
        assert_eq!(
            charge.layout(),
            member.public_key().retained_allocation_layout()
        );
        assert!(charge.belongs_to(&pool));
    }
    let exact = actual_controller_layouts(&account);
    assert_eq!(
        pool.reserved_bytes(),
        exact,
        "span scratch has already dropped"
    );
    let retained = parts.finish(policy.version(), policy.threshold()).unwrap();
    assert_eq!(retained.value, *policy);
    assert!(core::ptr::eq(retained.value.members().as_ptr(), original));
    assert_eq!(pool.reserved_bytes(), exact);
    pool.set_limit_bytes(0);
    drop(retained);
    assert_eq!(pool.reserved_bytes(), 0);
}

fn replace_amount(frame: &[u8], scale: u32) -> Vec<u8> {
    let view = ncore::from_bytes_view(frame).unwrap();
    let _flags = ncore::DecodeFlagsGuard::enter(view.flags());
    let _context = ncore::PayloadCtxGuard::enter(view.as_bytes());
    let mut offset = 0;
    let _source = ncore::framed_field::<AssetId>(view.as_bytes(), &mut offset).unwrap();
    let _destination = ncore::framed_field::<AccountId>(view.as_bytes(), &mut offset).unwrap();
    let amount_start = offset;
    let amount = ncore::framed_field::<Quantity>(view.as_bytes(), &mut offset).unwrap();
    let mut amount_offset = 0;
    // Quantity delegates directly to Numeric's positional mantissa/scale body,
    // so replace only the exact scale field without decoding/altering mantissa.
    let _mantissa =
        ncore::framed_field::<iroha_primitives::bigint::BigInt>(amount.bytes(), &mut amount_offset)
            .unwrap();
    let mut amount_body = amount.bytes()[..amount_offset].to_vec();
    let mut encoded_scale = Vec::new();
    ncore::serialize_to_buffer(&scale, &mut encoded_scale).unwrap();
    ncore::write_len_to_vec_with_flags(
        &mut amount_body,
        u64::try_from(encoded_scale.len()).unwrap(),
        view.flags(),
    );
    amount_body.extend_from_slice(&encoded_scale);
    let mut body = view.as_bytes()[..amount_start].to_vec();
    ncore::write_len_to_vec_with_flags(
        &mut body,
        u64::try_from(amount_body.len()).unwrap(),
        view.flags(),
    );
    body.extend_from_slice(&amount_body);
    ncore::frame_bare_with_header_flags::<AmxTransferLegV1>(&body, view.flags()).unwrap()
}

#[test]
fn funded_native_leg_cumulative_prefix_refusal_precedes_late_invalid_quantity_without_preview() {
    let original = leg(single(1), single(3), Quantity::from(17_u32));
    let frame = norito::encode_canonical(&original).unwrap();
    let late = replace_amount(&frame, u32::MAX);
    assert!(norito::decode_canonical::<AmxTransferLegV1>(&late).is_err());
    let observation = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 64);
    let first_key = key(1);
    let first_key_frame = norito::encode_canonical(&first_key).unwrap();
    let (decoded_key, first_key_usage) = ncore::with_decode_limits_measured(observation, || {
        norito::decode_canonical::<PublicKey>(&first_key_frame)
    });
    assert_eq!(decoded_key.unwrap(), first_key);
    assert!(first_key_usage.total_elements() > 0);
    let (valid, census) = ncore::with_decode_limits_measured(observation, || {
        norito::decode_canonical::<AmxTransferLegV1>(&frame)
    });
    assert_eq!(valid.unwrap(), original);
    let ceiling = census
        .total_elements()
        .checked_sub(1)
        .expect("both original keys consume successful work");
    assert!(ceiling >= key(1).retained_allocation_layout().size());
    let limits = DecodeLimits::new(usize::MAX, usize::MAX, ceiling, usize::MAX, 64);
    let ordinary = DecodeBudgetContext::new(limits);
    let actual = DecodeBudgetContext::new(limits);
    let shared = actual.clone();
    let pool = AllocationBudget::new(4096);
    let pending = PendingAmxTransferLegDecodeV1::new(&late, &pool);
    let mut first_prefix = 0;
    for (attempt, context) in [&actual, &shared].into_iter().enumerate() {
        let (expected, original_usage) = ncore::with_decode_limits_measured(observation, || {
            ordinary.with(|| norito::decode_canonical::<AmxTransferLegV1>(&late))
        });
        let expected = expected.unwrap_err();
        let (failure, usage) = ncore::with_decode_limits_measured(observation, || {
            context.with(|| pending.try_decode())
        });
        let failure = failure.unwrap_err();
        let Error::Decode(failure) = failure else {
            panic!("original logical refusal was erased")
        };
        assert_eq!(
            failure.kind(),
            ncore::DecodeAttemptErrorKind::EnclosingLimit
        );
        assert_eq!(
            pool.reserved_bytes(),
            PreparedDecodeWorkspace::allocation_layouts()[0].size(),
            "only the captured original counter remains; graph and second control have retired"
        );
        let failure = failure.into_error();
        assert!(
            failure.is_decode_resource_limit(),
            "late Quantity must not preempt earlier work"
        );
        assert_eq!(
            failure.decode_resource_error(),
            expected.decode_resource_error()
        );
        // Element work is identical. The prepared path intentionally avoids
        // ordinary alignment scratch, so its allocation-byte total differs.
        assert_eq!(usage.total_elements(), original_usage.total_elements());
        if attempt == 0 {
            assert!(
                usage.total_elements() > 0,
                "successful prefix must be consumed before late failure"
            );
            first_prefix = actual.consumed_allocated_bytes();
            assert!(first_prefix > 0);
        } else {
            assert_eq!(
                usage.total_elements(),
                first_key_usage.total_elements(),
                "the outer observer charges the first key before the shared context refuses it"
            );
            assert_eq!(
                actual.consumed_allocated_bytes(),
                first_prefix,
                "failed retry neither replenishes work nor allocates a refused prefix"
            );
        }
        // The opaque original resource error still owns its counter family.
        assert_eq!(
            pool.reserved_bytes(),
            PreparedDecodeWorkspace::allocation_layouts()[0].size()
        );
        drop(failure);
        assert_eq!(
            pool.reserved_bytes(),
            0,
            "physical credit refunds do not reset logical work"
        );
        assert!(core::ptr::eq(
            pending.original_source().as_ptr(),
            late.as_ptr()
        ));
    }
    let failure = pending.try_decode().unwrap_err();
    let Error::Decode(actual) = failure else {
        panic!("original scalar rejection")
    };
    assert_eq!(actual.kind(), ncore::DecodeAttemptErrorKind::Invalid);
    let expected = norito::decode_canonical::<AmxTransferLegV1>(&late).unwrap_err();
    assert_eq!(actual.to_string(), expected.to_string());
    drop(actual);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn funded_native_leg_rejects_original_malformed_frames_flags_and_payload_bound() {
    let original = leg(single(1), single(3), Quantity::from(17_u32));
    let frame = norito::encode_canonical(&original).unwrap();
    let pool = AllocationBudget::new(4096);
    for bytes in [&frame[..frame.len() - 1], &[]] {
        assert!(
            PendingAmxTransferLegDecodeV1::new(bytes, &pool)
                .try_decode()
                .is_err()
        );
        assert_eq!(pool.reserved_bytes(), 0);
    }
    let mut corrupted = frame.clone();
    *corrupted.last_mut().unwrap() ^= 1;
    let failure = PendingAmxTransferLegDecodeV1::new(&corrupted, &pool)
        .try_decode()
        .unwrap_err();
    let Error::Decode(error) = failure else {
        panic!("captured original checksum error")
    };
    assert!(matches!(
        error.into_error(),
        norito::Error::ChecksumMismatch
    ));
    let mut flags = frame.clone();
    flags[ncore::Header::SIZE - 1] = 0x80;
    assert!(
        PendingAmxTransferLegDecodeV1::new(&flags, &pool)
            .try_decode()
            .is_err()
    );
    let mut trailing = frame.clone();
    trailing.push(0);
    assert!(
        PendingAmxTransferLegDecodeV1::new(&trailing, &pool)
            .try_decode()
            .is_err()
    );
    let oversized = vec![0; MAX_RESULT_PREIMAGE_BYTES + 1];
    assert!(matches!(
        PendingAmxTransferLegDecodeV1::new(&oversized, &pool).try_decode(),
        Err(Error::Record(_))
    ));
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn funded_native_leg_preserves_canonical_header_error_order_without_crc_preview() {
    let original = leg(single(1), single(3), Quantity::from(17_u32));
    let frame = norito::encode_canonical(&original).unwrap();
    let pool = AllocationBudget::new(4096);
    // Header::write puts schema after magic and major/minor; CRC corruption
    // coexists with each earlier error. Ordinary typed admission is the oracle.
    let mut schema_and_crc = frame.clone();
    schema_and_crc[4 + 1 + 1] ^= 1;
    *schema_and_crc.last_mut().unwrap() ^= 1;
    let mut flags_schema_and_crc = schema_and_crc.clone();
    flags_schema_and_crc[ncore::Header::SIZE - 1] = 0x80;
    for bytes in [&schema_and_crc, &flags_schema_and_crc] {
        let ordinary = norito::decode_canonical::<AmxTransferLegV1>(bytes).unwrap_err();
        let failure = PendingAmxTransferLegDecodeV1::new(bytes, &pool)
            .try_decode()
            .unwrap_err();
        let Error::Decode(captured) = failure else {
            panic!("typed canonical cause must be captured")
        };
        assert_eq!(captured.kind(), ncore::DecodeAttemptErrorKind::Invalid);
        assert_eq!(captured.to_string(), ordinary.to_string());
        assert!(
            !matches!(ordinary, norito::Error::ChecksumMismatch),
            "CRC must not preempt earlier type/flag errors"
        );
        drop(captured);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn funded_native_leg_preserves_original_root_depth_field_and_source_context() {
    let original = leg(single(1), single(3), Quantity::from(17_u32));
    let frame = norito::encode_canonical(&original).unwrap();
    let pool = AllocationBudget::new(4096);
    let pending = PendingAmxTransferLegDecodeV1::new(&frame, &pool);
    let sentinel = [9_u8, 7, 5];
    let _source = ncore::PayloadCtxGuard::enter(&sentinel);
    let _flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    let context = ncore::payload_ctx();
    let flags = ncore::get_decode_flags();
    for ceiling in [
        DecodeLimits::new(usize::MAX, 1, usize::MAX, usize::MAX, 0),
        DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 0),
    ] {
        let ordinary = ncore::with_decode_limits_scope(ceiling, || {
            norito::decode_canonical_for_admission::<AmxTransferLegV1>(
                &frame,
                norito::canonical_decode_limits(frame.len()),
            )
        })
        .unwrap_err();
        let failure =
            ncore::with_decode_limits_scope(ceiling, || pending.try_decode()).unwrap_err();
        let Error::Decode(actual) = failure else {
            panic!("original root refusal")
        };
        assert_eq!(actual.kind(), ordinary.kind());
        assert_eq!(actual.kind(), ncore::DecodeAttemptErrorKind::EnclosingLimit);
        assert_eq!(
            pool.reserved_bytes(),
            PreparedDecodeWorkspace::allocation_layouts()[0].size()
        );
        let actual = actual.into_error();
        assert_eq!(
            actual.decode_resource_error(),
            ordinary.into_error().decode_resource_error()
        );
        assert_eq!(ncore::payload_ctx(), context);
        assert_eq!(ncore::get_decode_flags(), flags);
        assert!(core::ptr::eq(
            pending.original_source().as_ptr(),
            frame.as_ptr()
        ));
        drop(actual);
        assert_eq!(pool.reserved_bytes(), 0);
    }
    let owner = pending.try_decode().unwrap();
    assert_eq!(owner.canonical(), &original);
    assert_eq!(ncore::payload_ctx(), context);
    assert_eq!(ncore::get_decode_flags(), flags);
    drop(owner);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn funded_native_leg_destination_reset_retains_backing_and_refuses_replacement() {
    let original = leg(single(1), single(3), Quantity::from(17_u32));
    let frame = norito::encode_canonical(&original).unwrap();
    let retained = exact_retained(&original);
    let pool = AllocationBudget::new(retained + controls());
    let mut reservation = pool
        .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
        .unwrap();
    let mut workspace = PreparedDecodeWorkspace::from_reservation(&pool, &mut reservation).unwrap();
    drop(reservation);
    let mut destination = LegDestination::new(&pool);
    workspace
        .decode_canonical_into::<AmxTransferLegV1, _>(
            &frame,
            norito::canonical_decode_limits(frame.len()),
            &mut destination,
        )
        .unwrap();
    let backing = core::ptr::from_ref(destination.complete.as_ref().unwrap().canonical());
    let expected = retained + controls();
    assert_eq!(pool.reserved_bytes(), expected);
    destination.reset();
    assert!(!destination.valid);
    assert_eq!(
        core::ptr::from_ref(destination.complete.as_ref().unwrap().canonical()),
        backing
    );
    assert_eq!(
        pool.reserved_bytes(),
        expected,
        "reset does not retire or replace backing"
    );
    let failure = workspace
        .decode_canonical_into::<AmxTransferLegV1, _>(
            &frame,
            norito::canonical_decode_limits(frame.len()),
            &mut destination,
        )
        .unwrap_err();
    assert!(matches!(
        failure,
        PreparedDecodeError::Destination(Error::Invariant("populated native AMX leg destination"))
    ));
    assert_eq!(
        core::ptr::from_ref(destination.complete.as_ref().unwrap().canonical()),
        backing
    );
    assert_eq!(pool.reserved_bytes(), expected);
    assert!(!destination.valid);
    pool.set_limit_bytes(0);
    drop(destination);
    assert_eq!(
        pool.reserved_bytes(),
        controls(),
        "values and graph charges precede control retirement"
    );
    drop(workspace);
    assert_eq!(pool.reserved_bytes(), 0);
}

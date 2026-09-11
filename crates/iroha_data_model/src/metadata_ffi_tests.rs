//! Combined base/aggregate opaque-handle ownership and exported-symbol regressions.
#![allow(unsafe_code)]

use core::{cmp::Ordering, ffi::c_void, mem::MaybeUninit, ptr};
use iroha_ffi::{FfiConvert, FfiOutPtr, FfiOutPtrRead, FfiReturn, FfiType, Handle};
use iroha_model_base::metadata::Metadata;
use iroha_primitives::json::Json;

type HandleId = <iroha_ffi::handle::Id as FfiType>::ReprC;
type BoolOut = <bool as FfiOutPtr>::OutPtr;
type OrderingOut = <Ordering as FfiOutPtr>::OutPtr;
type DropFn = unsafe extern "C" fn(HandleId, *mut c_void) -> FfiReturn;
type CloneFn = unsafe extern "C" fn(HandleId, *const c_void, *mut *mut c_void) -> FfiReturn;
type EqFn = unsafe extern "C" fn(HandleId, *const c_void, *const c_void, *mut BoolOut) -> FfiReturn;
type OrdFn =
    unsafe extern "C" fn(HandleId, *const c_void, *const c_void, *mut OrderingOut) -> FfiReturn;

// Resolve the real exported symbols from both linked crates. Defining local
// dispatchers here would not detect stale or duplicate production ownership.
unsafe extern "C" {
    #[link_name = "iroha_model_base__drop"]
    fn base_drop(id: HandleId, value: *mut c_void) -> FfiReturn;
    #[link_name = "iroha_model_base__clone"]
    fn base_clone(id: HandleId, value: *const c_void, out: *mut *mut c_void) -> FfiReturn;
    #[link_name = "iroha_model_base__eq"]
    fn base_eq(
        id: HandleId,
        left: *const c_void,
        right: *const c_void,
        out: *mut BoolOut,
    ) -> FfiReturn;
    #[link_name = "iroha_model_base__ord"]
    fn base_ord(
        id: HandleId,
        left: *const c_void,
        right: *const c_void,
        out: *mut OrderingOut,
    ) -> FfiReturn;
    #[link_name = "iroha_data_model__drop"]
    fn aggregate_drop(id: HandleId, value: *mut c_void) -> FfiReturn;
    #[link_name = "iroha_data_model__clone"]
    fn aggregate_clone(id: HandleId, value: *const c_void, out: *mut *mut c_void) -> FfiReturn;
    #[link_name = "iroha_data_model__eq"]
    fn aggregate_eq(
        id: HandleId,
        left: *const c_void,
        right: *const c_void,
        out: *mut BoolOut,
    ) -> FfiReturn;
    #[link_name = "iroha_data_model__ord"]
    fn aggregate_ord(
        id: HandleId,
        left: *const c_void,
        right: *const c_void,
        out: *mut OrderingOut,
    ) -> FfiReturn;
    fn __dealloc(value: *mut u8, size: usize, align: usize) -> FfiReturn;
}

fn fixture() -> Metadata {
    let mut value = Metadata::default();
    value.insert("alpha".parse().unwrap(), Json::new("first"));
    value.insert("beta".parse().unwrap(), Json::new(vec![1_u64, 2, 3]));
    value
}

fn compare_through_exports(left: &Metadata, right: &Metadata) -> (bool, Ordering) {
    let left = left.into_ffi(&mut ());
    let right = right.into_ffi(&mut ());
    let id = Metadata::ID.into_ffi(&mut ());
    let mut equal = MaybeUninit::<BoolOut>::uninit();
    let mut ordering = MaybeUninit::<OrderingOut>::uninit();
    // SAFETY: both pointers borrow live Metadata values, ID 3 selects that
    // exact type, and the writable outputs have the canonical FFI layouts.
    unsafe {
        assert_eq!(
            base_eq(id, left.cast(), right.cast(), equal.as_mut_ptr()),
            FfiReturn::Ok
        );
        assert_eq!(
            base_ord(id, left.cast(), right.cast(), ordering.as_mut_ptr()),
            FfiReturn::Ok
        );
        (
            bool::try_read_out(equal.assume_init()).expect("valid FFI boolean"),
            Ordering::try_read_out(ordering.assume_init()).expect("valid FFI ordering"),
        )
    }
}

#[test]
fn metadata_move_preserves_all_assigned_handle_ids() {
    assert_eq!(<crate::account::Account as Handle>::ID, 0);
    assert_eq!(<crate::asset::value::Asset as Handle>::ID, 1);
    assert_eq!(<crate::domain::Domain as Handle>::ID, 2);
    assert_eq!(Metadata::ID, 3);
    assert_eq!(<crate::permission::Permission as Handle>::ID, 4);
    assert_eq!(<crate::role::Role as Handle>::ID, 5);
}

#[test]
fn base_metadata_exports_clone_compare_and_release_the_actual_value() {
    let expected = fixture();
    let source = (&expected).into_ffi(&mut ());
    let mut output = MaybeUninit::<*mut Metadata>::new(ptr::null_mut());
    // SAFETY: source borrows the exact handle type and output is one writable
    // pointer slot. A successful clone transfers a distinct allocation to us.
    let cloned = unsafe {
        assert_eq!(
            base_clone(
                Metadata::ID.into_ffi(&mut ()),
                source.cast(),
                output.as_mut_ptr().cast()
            ),
            FfiReturn::Ok,
        );
        let output = output.assume_init();
        assert!(!output.is_null());
        assert_ne!(output.cast_const(), source);
        Metadata::try_from_ffi(output, &mut ()).expect("take ownership of the exported clone")
    };
    assert_eq!(cloned, expected);
    assert_eq!(
        norito::to_bytes(&cloned).unwrap(),
        norito::to_bytes(&expected).unwrap()
    );
    assert_eq!(
        compare_through_exports(&expected, &cloned),
        (true, Ordering::Equal)
    );
    let empty = Metadata::default();
    for (left, right) in [(&empty, &cloned), (&cloned, &empty)] {
        assert_eq!(
            compare_through_exports(left, right),
            (left == right, left.cmp(right))
        );
    }
    let owned = cloned.into_ffi(&mut ());
    // SAFETY: ownership is transferred back once to its canonical Drop export;
    // no Rust owner or subsequent pointer access remains after this call.
    assert_eq!(
        unsafe { base_drop(Metadata::ID.into_ffi(&mut ()), owned.cast()) },
        FfiReturn::Ok
    );
}

fn assert_unknown_handle(id: HandleId, drop: DropFn, clone: CloneFn, equal: EqFn, order: OrdFn) {
    let mut cloned = ptr::null_mut();
    let mut eq_output: BoolOut = 2;
    let mut ord_output: OrderingOut = 2;
    // SAFETY: every tested ID is absent from this exact dispatch table. Unknown
    // IDs must be rejected before pointer access. Null inputs also prevent an
    // accidentally retained type from freeing a real allocation in this test.
    unsafe {
        assert_eq!(drop(id, ptr::null_mut()), FfiReturn::UnknownHandle);
        assert_eq!(
            clone(id, ptr::null(), &raw mut cloned),
            FfiReturn::UnknownHandle
        );
        assert_eq!(
            equal(id, ptr::null(), ptr::null(), &raw mut eq_output),
            FfiReturn::UnknownHandle
        );
        assert_eq!(
            order(id, ptr::null(), ptr::null(), &raw mut ord_output),
            FfiReturn::UnknownHandle
        );
    }
    assert!(
        cloned.is_null(),
        "rejection must not produce an owned value"
    );
    assert_eq!(eq_output, 2, "rejection must not write an equality result");
    assert_eq!(ord_output, 2, "rejection must not write an ordering result");
}

#[test]
fn base_and_aggregate_dispatchers_reject_foreign_handle_ids() {
    for id in [3_u8, u8::MAX] {
        assert_unknown_handle(
            id.into_ffi(&mut ()),
            aggregate_drop,
            aggregate_clone,
            aggregate_eq,
            aggregate_ord,
        );
    }
    for id in [0_u8, 1, 2, 4, 5, u8::MAX] {
        assert_unknown_handle(
            id.into_ffi(&mut ()),
            base_drop,
            base_clone,
            base_eq,
            base_ord,
        );
    }
    // A real value still dispatches successfully after all rejection paths.
    let value = fixture();
    assert_eq!(
        compare_through_exports(&value, &value),
        (true, Ordering::Equal)
    );
}

#[test]
fn combined_exports_retain_the_single_global_deallocator() {
    let layout = std::alloc::Layout::from_size_align(64, 8).unwrap();
    // SAFETY: the shared symbol receives precisely the pointer/layout returned
    // by this allocation. Linking both owners also rejects a duplicate export.
    unsafe {
        let allocation = std::alloc::alloc(layout);
        assert!(!allocation.is_null());
        assert_eq!(
            __dealloc(allocation, layout.size(), layout.align()),
            FfiReturn::Ok
        );
    }
}

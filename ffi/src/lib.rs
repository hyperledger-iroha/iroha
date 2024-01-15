#![allow(unsafe_code)]
#![no_std]

//! Structures and macros related to FFI and generation of FFI bindings. Any type that implements
//! [`FfiType`] can be used in the FFI bindings generated with [`ffi_export`]/[`ffi_import`]. It
//! is advisable to implement [`Ir`] and benefit from automatic implementation of [`FfiType`]

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};

use derive_more::Display;
use ir::{Ir, Transmute};
pub use iroha_ffi_derive::*;
use repr_c::{
    COutPtr, COutPtrRead, COutPtrWrite, CType, CTypeConvert, CWrapperType, Cloned, NonLocal,
};

pub mod handle;
pub mod ir;
pub mod option;
pub mod primitives;
pub mod repr_c;
pub mod slice;
mod std_impls;

/// A specialized `Result` type for FFI operations
pub type Result<T> = core::result::Result<T, FfiReturn>;

/// Represents the handle in an FFI context
///
/// # Safety
///
/// If two structures implement the same id, it may result in a void pointer being casted to the wrong type
pub unsafe trait Handle {
    /// Unique identifier of the handle. Most commonly, it is
    /// used to facilitate generic monomorphization over FFI
    const ID: handle::Id;
}

/// Robust type that conforms to C ABI and can be safely shared across FFI boundaries. This does
/// not guarantee the ABI compatibility of the referent for pointers. These pointers are opaque
///
/// # Safety
///
/// Type implementing the trait must be a robust type with a guaranteed C ABI. Care must be taken
/// not to dereference pointers whose referents don't implement `ReprC`; they are considered opaque
// NOTE: Type is `Copy` to indicate that there can be no ownership transfer
pub unsafe trait ReprC: Copy {}

/// A type that can be converted into some C type
pub trait FfiType {
    /// C type current type can be converted into
    type ReprC: ReprC;
}

/// Facilitates conversion of rust types to/from `ReprC` types.
pub trait FfiConvert<'itm, C: ReprC>: Sized {
    /// Type into which state can be stored during conversion from [`Self`]. Useful for
    /// returning owning heap allocated types or non-owning types that are not transmutable.
    /// Serves similar purpose as does context in a closure
    type RustStore: Default;

    /// Type into which state can be stored during conversion into [`Self`]. Useful for
    /// returning non-owning types that are not transmutable. Serves similar purpose as
    /// does context in a closure
    type FfiStore: Default;

    /// Perform the conversion from [`Self`] into [`Self::ReprC`]
    fn into_ffi(self, store: &'itm mut Self::RustStore) -> C;

    /// Perform the conversion from [`Self::ReprC`] into [`Self`]
    ///
    /// # Errors
    ///
    /// Check [`FfiReturn`]
    ///
    /// # Safety
    ///
    /// All conversions from a pointer must ensure pointer validity beforehand
    unsafe fn try_from_ffi(source: C, store: &'itm mut Self::FfiStore) -> Result<Self>;
}

/// The trait is used to replace the type in the wrapper function generated by [`ffi_import`].
///
/// Most notably, the necessity to replace the output type for a wrapper function arises when:
/// - the wrapper function returns a reference that uses store during conversion (i.e. that are cloned)
/// - the wrapper function takes/return an opaque type reference
pub trait FfiWrapperType {
    /// Type used instead of the input type in the wrapper function generated by `ffi_import`
    type InputType;
    /// Type used instead of the output type in the wrapper function generated by `ffi_import`
    type ReturnType;
}

/// Facilitates the use of [`Self`] as out-pointer.
pub trait FfiOutPtr: FfiType {
    /// Type of the out-pointer
    type OutPtr: ReprC;
}

/// Facilitates writing [`Self`] into [`Self::OutPtr`].
pub trait FfiOutPtrWrite: FfiOutPtr {
    /// Write the given rust value into the corresponding out-pointer
    ///
    /// # Safety
    ///
    /// [`*mut Self::OutPtr`] must be valid
    unsafe fn write_out(self, out_ptr: *mut Self::OutPtr);
}

/// Facilitates reading from [`Self::OutPtr`] out-pointer.
pub trait FfiOutPtrRead: FfiOutPtr + Sized {
    /// Read a rust value from the corresponding out-pointer
    ///
    /// # Errors
    ///
    /// Check [`FfiConvert::try_from_ffi`]
    ///
    /// # Safety
    ///
    /// Check [`FfiConvert::try_from_ffi`]
    unsafe fn try_read_out(out_ptr: Self::OutPtr) -> Result<Self>;
}

/// Reference that owns its referent. This struct is used when wrapper functions generated by
/// `ffi_import` return types that use store during FFI serialization (e.g. `&(u32, u32)`).
///
/// # Example
///
/// ```
/// #[iroha_ffi::ffi_import]
/// pub fn func_returns_non_local(a: &u32) -> &u32 {
///    a
/// }
///
/// #[iroha_ffi::ffi_import]
/// pub fn func_returns_local(a: &(u32, u32)) -> &(u32, u32) {
///    a
/// }
///
/// /* When expanded, `ffi_import` will replace the annotated functions with functions equivalent to the following:
/// pub fn func_returns_non_local(a: &u32) -> &u32 {
///     let mut in_store = ();
///     let a = a.into_ffi(a, &mut in_store);
///
///     let mut output = core::mem::MaybeUninit::uninit();
///     __func_returns_non_local(a, output.as_mut_ptr());
///
///     let out_store = ();
///     let output = output.assume_init();
///
///     // &u32 doesn't reference local scope of a function
///     let output = FfiConvert::try_from_ffi(output, &mut out_store);
///     output
/// }
///
/// pub fn func(a: &(u32, u32)) -> LocalRef<(u32, u32)> {
///     let mut in_store = Default::default();
///     let a = a.into_ffi(a, &mut in_store);
///
///     let mut output = core::mem::MaybeUninit::uninit();
///     __func(a, output.as_mut_ptr());
///
///     let output = output.assume_init();
///     let out_store = Default::default();

///     // &(u32, u32) references out_store which is defined locally
///     let output = FfiConvert::try_from_ffi(output, &mut out_store);
///     LocalRef(out_store.0, core::marker::PhantomData)
/// } */
/// ```
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct LocalRef<'data, R>(R, core::marker::PhantomData<&'data ()>);

/// Shared slice that owns its referent. This struct is used when wrapper functions generated by
/// `ffi_import` return types that use store during FFI serialization (e.g. `&[(u32, u32)]`).
///
/// # Example
///
/// ```
/// #[iroha_ffi::ffi_import]
/// pub fn func_returns_non_local(a: &[u32]) -> &[u32] {
///    a
/// }
///
/// #[iroha_ffi::ffi_import]
/// pub fn func_returns_local(a: &[(u32, u32)]) -> &[(u32, u32)] {
///    a
/// }
///
/// /* When expanded, `ffi_import` will replace the annotated functions with functions equivalent to the following:
/// pub fn func_returns_non_local(a: &[u32]) -> &[u32] {
///     let mut in_store = ();
///     let a = a.into_ffi(a, &mut in_store);
///
///     let mut output = core::mem::MaybeUninit::uninit();
///     __func_returns_non_local(a, output.as_mut_ptr());
///
///     let out_store = ();
///     let output = output.assume_init();
///
///     // &[u32] doesn't reference local scope of a function
///     let output = FfiConvert::try_from_ffi(output, &mut out_store);
///     output
/// }
///
/// pub fn func_returns_local(a: &[(u32, u32)]) -> LocalSlice<(u32, u32)> {
///     let mut in_store = Default::default();
///     let a = a.into_ffi(a, &mut in_store);
///
///     let mut output = core::mem::MaybeUninit::uninit();
///     __func_returns_local(a, output.as_mut_ptr());
///
///     let output = output.assume_init();
///     let out_store = Default::default();
///
///     // &[(u32, u32)] references out_store which is defined locally
///     let output = FfiConvert::try_from_ffi(output, &mut out_store);
///     LocalSlice(out_store.0, core::marker::PhantomData)
/// } */
/// ```
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct LocalSlice<'data, R>(alloc::boxed::Box<[R]>, core::marker::PhantomData<&'data ()>);

/// Result of execution of an FFI function
#[derive(Debug, Display, Clone, Copy, PartialEq, Eq)]
#[repr(i8)]
pub enum FfiReturn {
    /// The input argument provided to FFI function can't be converted into inner rust representation.
    ConversionFailed = -6,
    /// The input argument provided to FFI function contains a trap representation.
    TrapRepresentation = -5,
    /// FFI function execution panicked.
    UnrecoverableError = -4,
    /// Provided handle id doesn't match any known handles.
    UnknownHandle = -3,
    /// FFI function failed during the execution of the wrapped method on the provided handle.
    ExecutionFail = -2,
    /// The input argument provided to FFI function is a null pointer.
    ArgIsNull = -1,
    /// FFI function executed successfully.
    Ok = 0,
}

/// Macro for defining FFI types of a known category ([`Robust`] or [`Transmute`]).
/// The implementation for an FFI type of one of the categories incurs a lot of bloat that
/// is reduced by the use of this macro
///
/// # Safety
///
/// * If the type is [`Robust`], it derives [`ReprC`]. Check safety invariants for [`ReprC`]
/// * If the type is [`Transparent`], it derives [`Transmute`]. Check safety invariants for [`Transmute`]
///
/// # Example
///
/// ```
/// use iroha_ffi::{ffi_type, ReprC};
///
/// // Always use a type alias for inner types of transparent items so that if you make
/// // a change the unsafe code in [`ffi_type!`] will not compile, thus preventing UB
/// type NonNullInner<T> = *mut T;
/// type WrapperInner = u32;
///
/// #[repr(transparent)]
/// struct NonNull<T>(NonNullInner<T>);
///
/// #[repr(transparent)]
/// struct Wrapper(WrapperInner);
///
/// #[derive(Clone, Copy)]
/// #[repr(C)]
/// struct RobustStruct(u64, i32);
///
/// // SAFETY: Type is robust #[repr(C)]
/// unsafe impl ReprC for RobustStruct {}
/// ffi_type! { impl Robust for RobustStruct {} }
///
/// ffi_type! {
///     unsafe impl<T> Transparent for NonNull<T> {
///         type Target = NonNullInner<T>;
///
///         validation_fn=unsafe {|target: &NonNullInner<T>| !target.is_null()},
///         niche_value=core::ptr::null_mut(),
///     }
/// }
///
/// // Validation function is `|_| true` implicitly indicating
/// // this type is robust with respect to the wrapped type
/// ffi_type! {
///     unsafe impl Transparent for Wrapper {
///         type Target = WrapperInner;
///     }
/// }
/// ```
#[macro_export]
macro_rules! ffi_type {
    (impl $(<$($impl_generics: tt $(: $bounds: path)?),*>)? Robust for $ty: ty $(where $($where_ty:ty: $where_bound:path),* )? {} ) => {
        impl$(<$($impl_generics $(: $bounds)?),*>)? $crate::ir::Ir for $ty where Self: $crate::ReprC, $($($where_ty: $where_bound),*)? {
            type Type = $crate::ir::Robust;
        }

        // SAFETY: Robust type with a defined C representation by definition
        unsafe impl$(<$($impl_generics $(: $bounds)?),*>)? $crate::ir::InfallibleTransmute for $ty where Self: $crate::ReprC, $($($where_ty: $where_bound),*)? {}

        impl$(<$($impl_generics $(: $bounds)?),*>)? $crate::option::OptionIr for $ty where $($($where_ty: $where_bound),*)? {
            type Type = Option<$crate::option::WithoutNiche>;
        }

        impl<$($($impl_generics $(: $bounds)?),*)?> $crate::WrapperTypeOf<Self> for $ty where $($($where_ty: $where_bound),*)? {
            type Type = Self;
        }
    };
    (unsafe impl $(<$($impl_generics: tt $(: $bounds: path)?),*>)? Transparent for $ty: ty $(where $($where_ty:ty: $where_bound:path),* )? {
        type Target = $target:ty;

        validation_fn=unsafe {$validity_fn: expr},
        niche_value=$niche_value: expr
        $(,)?
    }) => {
        impl<$($($impl_generics $(: $bounds)?),*)?> $crate::ir::Ir for $ty where $($($where_ty: $where_bound),*)? {
            type Type = $crate::ir::Transparent;
        }

        // SAFETY: `$ty` is transmutable into `$target` and `is_valid` doesn't return false positives
        unsafe impl<$($($impl_generics $(: $bounds)?),*)?> $crate::ir::Transmute for $ty where $($($where_ty: $where_bound),*)? {
            type Target = $target;

            #[inline]
            #[allow(clippy::redundant_closure_call)]
            unsafe fn is_valid(target: &Self::Target) -> bool {
                $validity_fn(target)
            }
        }

        impl<$($($impl_generics $(: $bounds)?),*)?> $crate::option::Niche<'_> for $ty where $($($where_ty: $where_bound),*)? {
            const NICHE_VALUE: <$target as $crate::FfiType>::ReprC = $niche_value;
        }
    };
    (unsafe impl $(<$($impl_generics: tt $(: $bounds: path)?),*>)? Transparent for $ty: ty $(where $($where_ty:ty: $where_bound:path),* )? {
        type Target = $target:ty;
    } ) => {
        impl<$($($impl_generics $(: $bounds)?),*)?> $crate::ir::Ir for $ty where $($($where_ty: $where_bound),*)? {
            type Type = $crate::ir::Transparent;
        }

        // SAFETY: `$ty` is transmutable into `$target` and `is_valid` doesn't return false positives
        unsafe impl<$($($impl_generics $(: $bounds)?),*)?> $crate::ir::Transmute for $ty where $($($where_ty: $where_bound),*)? {
            type Target = $target;

            #[inline]
            unsafe fn is_valid(_: &Self::Target) -> bool {
                true
            }
        }

        // SAFETY: `$t` is robust with respect to `$target`
        unsafe impl<$($($impl_generics $(: $bounds)?),*)?> $crate::ir::InfallibleTransmute for $ty where $($($where_ty: $where_bound),*)? {}

        impl<'dummy, $($($impl_generics $(: $bounds)?),*)?> $crate::option::Niche<'dummy> for $ty where $target: $crate::option::Niche<'dummy>, $($($where_ty: $where_bound),*)? {
            const NICHE_VALUE: <$target as $crate::FfiType>::ReprC = <$target as $crate::option::Niche<'dummy>>::NICHE_VALUE;
        }

        impl<$($($impl_generics $(: $bounds)?),*)?> $crate::WrapperTypeOf<$ty> for $target where $($($where_ty: $where_bound),*)? {
            type Type = $ty;
        }
    };
}

/// Wrapper around struct/enum opaque pointer. When wrapped with the [`ffi`] macro in the
/// crate linking dynamically to some `cdylib` crate, it replaces struct/enum body definition
#[repr(C)]
// NOTE: Irrelevant for a type that is always behind a pointer
#[allow(missing_copy_implementations)]
pub struct Extern {
    __data: [u8; 0],

    // Required for !Send & !Sync & !Unpin.
    //
    // - `*mut u8` is !Send & !Sync. It must be in `PhantomData` to not
    //   affect alignment.
    //
    // - `PhantomPinned` is !Unpin. It must be in `PhantomData` because
    //   its memory representation is not considered FFI-safe.
    __marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

/// Define the correct [`FfiWrapperType::InputType`]/[`FfiWrapperType::ReturnType`] out of
/// the given [`CWrapperType::InputType`]/[`CWrapperType::ReturnType`]. The only situation
/// when this is evident is when [`Ir::Type`] is set to [`Transparent`] or [`Extern`] types
///
/// Example:
///
/// ```
/// use iroha_ffi::FfiType;
///
/// #[derive(FfiType)]
/// #[ffi_type(unsafe {robust})]
/// #[repr(transparent)]
/// pub struct Example(u32);
///
/// /*
/// Due to the fact that implementations of traits for [`Transparent`] structures delegate to the inner
/// type `FfiType`, macro expansion will produce the following impl of [`CWrapperType`] for `Example`:
///
/// impl CWrapperType for Example {
///     type InputType = u32;
///     type ReturnType = u32;
/// }
///
/// which is corrected via [`WrapperTypeOf`] so that functions generated by [`ffi_import`] return [`Example`]
/// */
/// ```
pub trait WrapperTypeOf<T> {
    /// Correct return type of `T` in a function generated via [`ffi_import`]
    // TODO: Is associated type necessary if we already have a generic?
    type Type;
}

impl<R> WrapperTypeOf<Self> for *const R {
    type Type = Self;
}
impl<R> WrapperTypeOf<Self> for *mut R {
    type Type = Self;
}
impl<'itm, T> WrapperTypeOf<&'itm T> for *const T {
    type Type = &'itm T;
}
impl<'itm, T> WrapperTypeOf<&'itm mut T> for *mut T {
    type Type = &'itm mut T;
}
impl<'itm, R: ?Sized, T: ?Sized> WrapperTypeOf<&'itm R> for &'itm T {
    type Type = &'itm R;
}
impl<'itm, R: ?Sized, T: ?Sized> WrapperTypeOf<&'itm mut R> for &'itm mut T {
    type Type = &'itm mut R;
}
impl<R, T> WrapperTypeOf<Box<R>> for Box<T> {
    type Type = Box<R>;
}
impl<R, T> WrapperTypeOf<Vec<R>> for Vec<T> {
    type Type = Vec<R>;
}
impl<R, T, const N: usize> WrapperTypeOf<[R; N]> for [T; N] {
    type Type = [R; N];
}
impl<R, T> WrapperTypeOf<Option<R>> for Option<T> {
    type Type = Option<R>;
}

impl<'itm, R, T> WrapperTypeOf<&'itm R> for LocalRef<'itm, T> {
    type Type = LocalRef<'itm, R>;
}
impl<'slice, R, T> WrapperTypeOf<&'slice [R]> for LocalSlice<'slice, T> {
    type Type = LocalSlice<'slice, R>;
}

// SAFETY: `LocalRef` is transparent and `R` is transmutable into `R::Target`
unsafe impl<'itm, R: Transmute> Transmute for LocalRef<'itm, R> {
    type Target = LocalRef<'itm, R::Target>;

    #[inline]
    unsafe fn is_valid(target: &Self::Target) -> bool {
        R::is_valid(&target.0)
    }
}
// SAFETY: `LocalSlice` is transparent and `R` is transmutable into `R::Target`
unsafe impl<'itm, R: Transmute> Transmute for LocalSlice<'itm, R> {
    type Target = LocalSlice<'itm, R::Target>;

    #[inline]
    unsafe fn is_valid(target: &Self::Target) -> bool {
        target.iter().all(|item| R::is_valid(item))
    }
}

impl<R> core::ops::Deref for LocalRef<'_, R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<R> core::ops::Deref for LocalSlice<'_, R> {
    type Target = [R];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// SAFETY: `*const R` is robust with a defined C ABI regardless of whether `R` is
// When `R` is not `ReprC` the pointer is opaque; dereferencing is immediate UB
unsafe impl<R> ReprC for *const R {}
// SAFETY: `*mut R` is robust with a defined C ABI regardless of whether `R` is
// When `R` is not `ReprC` the pointer is opaque; dereferencing is immediate UB
unsafe impl<R> ReprC for *mut R {}
// SAFETY: `*mut R` is robust with a defined C ABI
unsafe impl<C: ReprC, const N: usize> ReprC for [C; N] {}

impl<R: Ir + CType<R::Type>> FfiType for R {
    type ReprC = <R as CType<R::Type>>::ReprC;
}
impl<'itm, R: Ir + CTypeConvert<'itm, R::Type, C>, C: ReprC> FfiConvert<'itm, C> for R {
    type RustStore = R::RustStore;
    type FfiStore = R::FfiStore;

    #[inline]
    fn into_ffi(self, store: &'itm mut Self::RustStore) -> C {
        self.into_repr_c(store)
    }

    #[inline]
    unsafe fn try_from_ffi(source: C, store: &'itm mut Self::FfiStore) -> Result<Self> {
        R::try_from_repr_c(source, store)
    }
}

impl FfiWrapperType for () {
    type InputType = ();
    type ReturnType = ();
}
impl<R: Ir + CWrapperType<R::Type>> FfiWrapperType for R {
    type InputType = R::InputType;
    type ReturnType = R::ReturnType;
}
impl<R: Ir + COutPtr<R::Type>> FfiOutPtr for R {
    type OutPtr = R::OutPtr;
}
impl<R: Ir + COutPtrWrite<R::Type>> FfiOutPtrWrite for R {
    unsafe fn write_out(self, out_ptr: *mut Self::OutPtr) {
        R::write_out(self, out_ptr);
    }
}
impl<R: Ir + COutPtrRead<R::Type>> FfiOutPtrRead for R {
    unsafe fn try_read_out(out_ptr: Self::OutPtr) -> Result<Self> {
        R::try_read_out(out_ptr)
    }
}

macro_rules! impl_tuple {
    ( ($( $ty:ident ),+) -> $ffi_ty:ident with ($($repr_c:ident),+) ) => {
        /// FFI-compatible tuple with n elements
        #[repr(C)]
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $ffi_ty<$($ty: ReprC),+>($(pub $ty),+);

        #[allow(non_snake_case)]
        impl<$($ty: $crate::ReprC),+> From<($( $ty, )+)> for $ffi_ty<$($ty),+> {
            fn from(source: ($( $ty, )+)) -> Self {
                let ($($ty,)+) = source;
                Self($( $ty ),+)
            }
        }

        // SAFETY: Implementing type is robust with a defined C ABI
        unsafe impl<$($ty: ReprC),+> ReprC for $ffi_ty<$($ty),+> {}

        impl<$($ty),+> $crate::ir::Ir for ($($ty,)+) {
            type Type = Self;
        }

        impl<$($ty: FfiType),+> $crate::repr_c::CType<Self> for ($($ty,)+) {
            type ReprC = $ffi_ty<$($ty::ReprC),+>;
        }
        impl<'itm, $($ty: FfiConvert<'itm, $repr_c>, $repr_c: $crate::ReprC),+> $crate::repr_c::CTypeConvert<'itm, Self, $ffi_ty<$($repr_c),+>> for ($($ty,)+) {
            type RustStore = ($( $ty::RustStore, )+);
            type FfiStore = ($( $ty::FfiStore, )+);

            #[allow(non_snake_case)]
            fn into_repr_c(self, store: &'itm mut Self::RustStore) -> $ffi_ty<$($repr_c),+> {
                impl_tuple! {@decl_priv_store $($ty),+ for RustStore with $($repr_c),+}

                let ($($ty,)+) = self;
                let store: private_store::Store<$($ty),+, $($repr_c),+> = store.into();
                $ffi_ty($( <$ty as FfiConvert<$repr_c>>::into_ffi($ty, store.$ty),)+)
            }
            #[allow(non_snake_case, clippy::missing_errors_doc, clippy::missing_safety_doc)]
            unsafe fn try_from_repr_c(source: $ffi_ty<$($repr_c),+>, store: &'itm mut Self::FfiStore) -> Result<Self> {
                impl_tuple! {@decl_priv_store $($ty),+ for FfiStore with $($repr_c),+}

                let $ffi_ty($($ty,)+) = source;
                let store: private_store::Store<$($ty),+, $($repr_c),+> = store.into();
                Ok(($( <$ty as FfiConvert<$repr_c>>::try_from_ffi($ty, store.$ty)?, )+))
            }
        }

        impl<$($ty),+> CWrapperType<Self> for ($($ty,)+) {
            type InputType = Self;
            type ReturnType = Self;
        }
        impl<$($ty: FfiOutPtr),+> COutPtr<Self> for ($($ty,)+) {
            type OutPtr = $ffi_ty<$($ty::OutPtr),+>;
        }
        #[allow(non_snake_case)]
        impl<$($ty: FfiOutPtrWrite),+> COutPtrWrite<Self> for ($($ty,)+) {
            unsafe fn write_out(self, out_ptr: *mut Self::OutPtr) {
                impl_tuple! {@decl_priv_out_ptr $($ty),+}
                let mut field_out_ptrs = ($(core::mem::MaybeUninit::<$ty::OutPtr>::uninit(),)+);

                let ($($ty,)+) = self;
                let field_out_ptrs: private_out_ptr::OutPtr<$($ty),+> = (&mut field_out_ptrs).into();
                $( FfiOutPtrWrite::write_out($ty, field_out_ptrs.$ty.as_mut_ptr()); )+
                out_ptr.write($ffi_ty($( unsafe { field_out_ptrs.$ty.assume_init() } ),+));
            }
        }
        #[allow(non_snake_case)]
        impl<$($ty: FfiOutPtrRead),+> COutPtrRead<Self> for ($($ty,)+) {
            unsafe fn try_read_out(source: Self::OutPtr) -> Result<Self> {
                impl_tuple! {@decl_priv_out_ptr $($ty),+}

                let $ffi_ty($($ty,)+) = source;
                Ok(($( FfiOutPtrRead::try_read_out($ty)?, )+))
            }
        }

        impl<$($ty),+> WrapperTypeOf<Self> for ($($ty,)+) {
            type Type = Self;
        }

        impl<$($ty),+> Cloned for ($($ty,)+) where Self: CType<Self> {}

        // SAFETY: Tuple doesn't use store if it's inner types don't use it
        unsafe impl<$($ty: Ir + NonLocal<$ty::Type>),+> NonLocal<Self> for ($($ty,)+) {}
    };

    // NOTE: This is a trick to index tuples
    ( @decl_priv_store $( $ty:ident ),+ for $store:ident with $($repr_c:ident),+ ) => {
        mod private_store {
            pub struct Store<'itm, $($ty: $crate::FfiConvert<'itm, $repr_c>),+, $($repr_c: $crate::ReprC),+> {
                $(pub $ty: &'itm mut $ty::$store),+
            }

            impl<'itm, $($ty: $crate::FfiConvert<'itm, $repr_c>),+, $($repr_c: $crate::ReprC),+> From<&'itm mut ($($ty::$store,)+)> for Store<'itm, $($ty,)+ $($repr_c),+> {
                fn from(($($ty,)+): &'itm mut ($($ty::$store,)+)) -> Self {
                    Self {$($ty,)+}
                }
            }
        }
    };

    // NOTE: This is a trick to index tuples
    ( @decl_priv_out_ptr $( $ty:ident ),+ $(,)? ) => {
        mod private_out_ptr {
            pub struct OutPtr<'itm, $($ty: $crate::FfiOutPtrWrite),+> {
                $(pub $ty: &'itm mut core::mem::MaybeUninit::<$ty::OutPtr>),+
            }

            impl<'itm, $($ty: $crate::FfiOutPtrWrite),+> From<&'itm mut ($(core::mem::MaybeUninit::<$ty::OutPtr>,)+)> for OutPtr<'itm, $($ty),+> {
                fn from(($($ty,)+): &'itm mut ($(core::mem::MaybeUninit::<$ty::OutPtr>,)+)) -> Self {
                    Self {$($ty,)+}
                }
            }
        }
    };
}

impl_tuple! {(A) -> FfiTuple1 with (C1)}
impl_tuple! {(A, B) -> FfiTuple2 with (C1, C2)}
impl_tuple! {(A, B, C) -> FfiTuple3 with (C1, C2, C3)}
impl_tuple! {(A, B, C, D) -> FfiTuple4 with (C1, C2, C3, C4)}
impl_tuple! {(A, B, C, D, E) -> FfiTuple5 with (C1, C2, C3, C4, C5)}
impl_tuple! {(A, B, C, D, E, F) -> FfiTuple6 with (C1, C2, C3, C4, C5, C6)}
impl_tuple! {(A, B, C, D, E, F, G) -> FfiTuple7 with (C1, C2, C3, C4, C5, C6, C7)}
impl_tuple! {(A, B, C, D, E, F, G, H) -> FfiTuple8 with (C1, C2, C3, C4, C5, C6, C7, C8)}
impl_tuple! {(A, B, C, D, E, F, G, H, I) -> FfiTuple9 with (C1, C2, C3, C4, C5, C6, C7, C8, C9)}
impl_tuple! {(A, B, C, D, E, F, G, H, I, J) -> FfiTuple10 with (C1, C2, C3, C4, C5, C6, C7, C8, C9, C10)}
impl_tuple! {(A, B, C, D, E, F, G, H, I, J, K) -> FfiTuple11 with (C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11)}
impl_tuple! {(A, B, C, D, E, F, G, H, I, J, K, L) -> FfiTuple12 with (C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11, C12)}

//! Logic related to the conversion of [`Option<T>`] to and from FFI-compatible representation

use core::mem::MaybeUninit;
use std::boxed::Box;

use crate::{
    FfiConvert, FfiOutPtr, FfiOutPtrRead, FfiOutPtrWrite, FfiReturn, FfiType, FfiWrapperType,
    ReprC, Result,
    ir::Ir,
    repr_c::{
        COutPtr, COutPtrRead, COutPtrWrite, CType, CTypeConvert, CWrapperType, Cloned, NonLocal,
    },
};

/// Marker for [`Option<T>`] that doesn't have niche representation
#[derive(Debug, Clone, Copy)]
pub enum WithoutNiche {}

/// Used to implement specialized impls of [`Ir`] for [`Option<T>`]
pub trait OptionIr {
    /// Internal representation of [`Option<T>`]
    type Type;
}

impl<'dummy, R: Niche<'dummy>> OptionIr for R {
    type Type = Option<Self>;
}

/// FFI representation used when the wrapped type does not expose a niche.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OptionWithoutNicheRepr<T: ReprC> {
    /// Discriminant encoded via the `u8` FFI representation.
    pub discriminant: <u8 as FfiType>::ReprC,
    /// Stored value; left uninitialised for the `None` variant.
    pub value: MaybeUninit<T>,
}

unsafe impl<T: ReprC> ReprC for OptionWithoutNicheRepr<T> {}

// `Option` values are cloned when bridged via references; see module tests for coverage.
impl<R> Cloned for Option<R> {}

/// Type that has at least one trap representation that can be used as a niche value. The
/// niche value is used in the serialization of [`Option<T>`].
///
/// For example, [`Option<bool>`] will be serilized into one byte
/// and [`Option<*const T>`] will take the size of the pointer
// The explicit lifetime parameter acts as a workaround for
// https://github.com/rust-lang/rust/issues/48214, giving the proc-macro
// enough structure to avoid inference pitfalls when deriving `FfiType` for
// references. Keep the lifetime until the upstream issue is resolved.
pub trait Niche<'dummy>: FfiType {
    /// The niche value of the type
    const NICHE_VALUE: Self::ReprC;
}

impl<R, C> Niche<'_> for &R
where
    Self: FfiType<ReprC = *const C>,
{
    const NICHE_VALUE: Self::ReprC = core::ptr::null();
}

impl<R, C> Niche<'_> for &mut R
where
    Self: FfiType<ReprC = *mut C>,
{
    const NICHE_VALUE: Self::ReprC = core::ptr::null_mut();
}

impl<R, C> Niche<'_> for Box<R>
where
    Self: FfiType<ReprC = *mut C>,
{
    const NICHE_VALUE: Self::ReprC = core::ptr::null_mut();
}

impl<R: OptionIr> Ir for Option<R> {
    type Type = R::Type;
}

impl<'dummy, R: Niche<'dummy>> CType<Self> for Option<R> {
    type ReprC = R::ReprC;
}
impl<'dummy, 'itm, R: Niche<'dummy, ReprC = C> + FfiConvert<'itm, C>, C: ReprC>
    CTypeConvert<'itm, Self, C> for Option<R>
where
    R::ReprC: PartialEq,
{
    type RustStore = R::RustStore;
    type FfiStore = R::FfiStore;

    fn into_repr_c(self, store: &'itm mut Self::RustStore) -> C {
        if let Some(value) = self {
            return value.into_ffi(store);
        }

        R::NICHE_VALUE
    }

    unsafe fn try_from_repr_c(source: C, store: &'itm mut Self::FfiStore) -> Result<Self> {
        if source == R::NICHE_VALUE {
            return Ok(None);
        }

        let value = unsafe { R::try_from_ffi(source, store)? };
        Ok(Some(value))
    }
}

impl<R: FfiWrapperType> CWrapperType<Self> for Option<R> {
    type InputType = Option<R::InputType>;
    type ReturnType = Option<R::ReturnType>;
}
impl<'dummy, R: Niche<'dummy> + FfiOutPtr> COutPtr<Self> for Option<R> {
    type OutPtr = R::OutPtr;
}
impl<'dummy, R: Niche<'dummy> + FfiOutPtrWrite<OutPtr = <R as FfiType>::ReprC>> COutPtrWrite<Self>
    for Option<R>
{
    unsafe fn write_out(self, out_ptr: *mut Self::OutPtr) {
        self.map_or_else(
            || unsafe { out_ptr.write(R::NICHE_VALUE) },
            |value| unsafe { R::write_out(value, out_ptr) },
        );
    }
}
impl<'dummy, R: Niche<'dummy> + FfiOutPtrRead<OutPtr = <R as FfiType>::ReprC>> COutPtrRead<Self>
    for Option<R>
where
    R::ReprC: PartialEq,
{
    unsafe fn try_read_out(out_ptr: Self::OutPtr) -> Result<Self> {
        if out_ptr == R::NICHE_VALUE {
            return Ok(None);
        }

        unsafe { R::try_read_out(out_ptr).map(Some) }
    }
}

impl<R: FfiType> CType<Option<WithoutNiche>> for Option<R> {
    type ReprC = OptionWithoutNicheRepr<R::ReprC>;
}

impl<'itm, R: FfiConvert<'itm, C>, C: ReprC>
    CTypeConvert<'itm, Option<WithoutNiche>, OptionWithoutNicheRepr<C>> for Option<R>
{
    type RustStore = R::RustStore;
    type FfiStore = R::FfiStore;

    fn into_repr_c(self, store: &'itm mut Self::RustStore) -> OptionWithoutNicheRepr<C> {
        // NOTE: Makes the code much more readable
        #[allow(clippy::option_if_let_else)]
        match self {
            None => OptionWithoutNicheRepr {
                discriminant: 0u8.into_ffi(&mut ()),
                value: MaybeUninit::uninit(),
            },
            Some(value) => OptionWithoutNicheRepr {
                discriminant: 1u8.into_ffi(&mut ()),
                value: MaybeUninit::new(value.into_ffi(store)),
            },
        }
    }

    unsafe fn try_from_repr_c(
        source: OptionWithoutNicheRepr<C>,
        store: &'itm mut Self::FfiStore,
    ) -> Result<Self> {
        let tag = unsafe { u8::try_from_ffi(source.discriminant, &mut ())? };
        match tag {
            0 => Ok(None),
            1 => {
                let value = unsafe { R::try_from_ffi(source.value.assume_init(), store)? };
                Ok(Some(value))
            }
            _ => Err(FfiReturn::TrapRepresentation),
        }
    }
}

impl<R: FfiWrapperType> CWrapperType<Option<WithoutNiche>> for Option<R> {
    type InputType = Option<R::InputType>;
    type ReturnType = Option<R::ReturnType>;
}

impl<R: FfiOutPtr> COutPtr<Option<WithoutNiche>> for Option<R> {
    type OutPtr = OptionWithoutNicheRepr<R::OutPtr>;
}

impl<R: FfiOutPtrWrite> COutPtrWrite<Option<WithoutNiche>> for Option<R> {
    unsafe fn write_out(self, out_ptr: *mut Self::OutPtr) {
        #[allow(clippy::option_if_let_else)]
        match self {
            None => {
                let mut discriminant_out_ptr = MaybeUninit::uninit();
                unsafe { FfiOutPtrWrite::write_out(0u8, discriminant_out_ptr.as_mut_ptr()) };
                let discriminant_out_ptr = unsafe { discriminant_out_ptr.assume_init() };
                let discriminant = <u8 as FfiType>::ReprC::from(discriminant_out_ptr);

                unsafe {
                    out_ptr.write(OptionWithoutNicheRepr {
                        discriminant,
                        value: MaybeUninit::uninit(),
                    });
                }
            }
            Some(value) => {
                let mut discriminant_out_ptr = MaybeUninit::uninit();
                unsafe { FfiOutPtrWrite::write_out(1u8, discriminant_out_ptr.as_mut_ptr()) };
                let discriminant_out_ptr = unsafe { discriminant_out_ptr.assume_init() };
                let discriminant = <u8 as FfiType>::ReprC::from(discriminant_out_ptr);

                let mut value_out_ptr = MaybeUninit::uninit();
                unsafe { FfiOutPtrWrite::write_out(value, value_out_ptr.as_mut_ptr()) };
                let value_out_ptr = unsafe { value_out_ptr.assume_init() };

                unsafe {
                    out_ptr.write(OptionWithoutNicheRepr {
                        discriminant,
                        value: MaybeUninit::new(value_out_ptr),
                    });
                }
            }
        }
    }
}

impl<R: FfiOutPtrRead> COutPtrRead<Option<WithoutNiche>> for Option<R> {
    unsafe fn try_read_out(out_ptr: Self::OutPtr) -> Result<Self> {
        let tag = unsafe { u8::try_from_ffi(out_ptr.discriminant, &mut ())? };
        match tag {
            0 => Ok(None),
            1 => {
                let value_ptr = unsafe { out_ptr.value.assume_init() };
                Ok(Some(unsafe { R::try_read_out(value_ptr)? }))
            }
            _ => Err(FfiReturn::TrapRepresentation),
        }
    }
}

// SAFETY: Option<Tdoesn't use store if it's inner types don't use it
unsafe impl<'dummy, R: Niche<'dummy> + Ir + NonLocal<R::Type>> NonLocal<Self> for Option<R> {}
// SAFETY: Option<Tdoesn't use store if it's inner types don't use it
unsafe impl<R: Ir + NonLocal<R::Type>> NonLocal<Option<WithoutNiche>> for Option<R> {}

#[cfg(test)]
mod tests {
    use core::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::repr_c::CTypeConvert;

    static CLONE_COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[repr(transparent)]
    #[derive(Debug, PartialEq, Eq)]
    struct CloneProbe(u32);

    impl Clone for CloneProbe {
        fn clone(&self) -> Self {
            CLONE_COUNTER.fetch_add(1, Ordering::Relaxed);
            Self(self.0)
        }
    }

    crate::ffi_type! {
        unsafe impl Transparent for CloneProbe {
            type Target = u32;
        }
    }

    type InnerRepr = OptionWithoutNicheRepr<<CloneProbe as FfiType>::ReprC>;
    type RefRepr = *const InnerRepr;

    fn reset_counter() {
        CLONE_COUNTER.store(0, Ordering::Relaxed);
    }

    fn assert_counter(expected: usize) {
        assert_eq!(CLONE_COUNTER.load(Ordering::Relaxed), expected);
    }

    #[test]
    fn reference_serialization_clones_some_variant() {
        reset_counter();
        let value = Some(CloneProbe(7));
        let mut store = <&Option<CloneProbe> as CTypeConvert<
            '_,
            &Option<WithoutNiche>,
            RefRepr,
        >>::RustStore::default();
        let ptr =
            <&Option<CloneProbe> as CTypeConvert<'_, &Option<WithoutNiche>, RefRepr>>::into_repr_c(
                &value, &mut store,
            );
        assert_counter(1);

        let mut ffi_store = <&Option<CloneProbe> as CTypeConvert<
            '_,
            &Option<WithoutNiche>,
            RefRepr,
        >>::FfiStore::default();
        let round_trip = unsafe {
            <&Option<CloneProbe> as CTypeConvert<
                '_,
                &Option<WithoutNiche>,
                RefRepr,
            >>::try_from_repr_c(ptr, &mut ffi_store)
            .expect("round trip")
        };
        assert_eq!(*round_trip, value);
        assert_counter(2);
    }

    #[test]
    fn reference_serialization_skips_clone_for_none() {
        reset_counter();
        let value: Option<CloneProbe> = None;
        let mut store = <&Option<CloneProbe> as CTypeConvert<
            '_,
            &Option<WithoutNiche>,
            RefRepr,
        >>::RustStore::default();
        let ptr =
            <&Option<CloneProbe> as CTypeConvert<'_, &Option<WithoutNiche>, RefRepr>>::into_repr_c(
                &value, &mut store,
            );
        assert_counter(0);

        let mut ffi_store = <&Option<CloneProbe> as CTypeConvert<
            '_,
            &Option<WithoutNiche>,
            RefRepr,
        >>::FfiStore::default();
        let round_trip = unsafe {
            <&Option<CloneProbe> as CTypeConvert<
                '_,
                &Option<WithoutNiche>,
                RefRepr,
            >>::try_from_repr_c(ptr, &mut ffi_store)
            .expect("round trip")
        };
        assert!(round_trip.is_none());
        assert_counter(0);
    }
}

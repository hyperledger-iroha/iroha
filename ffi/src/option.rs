//! Logic related to the conversion of [`Option<T>`] to and from FFI-compatible representation

use crate::{
    ir::Ir,
    repr_c::{COutPtr, CType, CTypeConvert, NonLocal},
    FfiConvert, FfiOutPtr, FfiType, ReprC, Result,
};

/// Type that has at least one trap representation that can be used as a niche value. The
/// niche value is used in the serialization of [`Option<T>`]. For example, [`Option<bool>`]
/// will be serilized into one byte and [`Option<*const T>`] will take the size of the pointer
pub trait Niche: FfiType {
    /// The niche value of the type
    const NICHE_VALUE: Self::ReprC;
}

impl<'itm, R, C> Niche for &'itm R
where
    Self: FfiType<ReprC = *const C>,
{
    const NICHE_VALUE: Self::ReprC = core::ptr::null();
}

impl<'itm, R, C> Niche for &'itm mut R
where
    Self: FfiType<ReprC = *mut C>,
{
    const NICHE_VALUE: Self::ReprC = core::ptr::null_mut();
}

impl<R: Niche> Ir for Option<R> {
    type Type = Option<R>;
}

impl<R: Niche> CType<Option<R>> for Option<R> {
    type ReprC = R::ReprC;
}
// TODO: Hopefully, compiler will elide checks for Option<&T>, Option<&mut T>, Option<*const T>
// if not U parameter can be used to distinguish set them apart from other type conversions
impl<'itm, R: FfiConvert<'itm, C> + Niche<ReprC = C>, C: ReprC> CTypeConvert<'itm, Option<R>, C>
    for Option<R>
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

        Ok(Some(R::try_from_ffi(source, store)?))
    }
}

impl<R: FfiOutPtr + Niche> COutPtr<Option<R>> for Option<R> {
    type OutPtr = R::OutPtr;
}

// SAFETY: Option<T> with a niche doesn't use store if it's inner types don't use it
unsafe impl<R: Niche + Ir + NonLocal<R::Type>> NonLocal<Option<R>> for Option<R> {}

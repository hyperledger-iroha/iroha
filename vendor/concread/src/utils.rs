use std::borrow::Borrow;
use std::cmp::Ordering;
// use std::mem::MaybeUninit;
#[cfg(feature = "serde")]
use std::fmt;
#[cfg(feature = "serde")]
use std::iter;
#[cfg(feature = "serde")]
use std::marker::PhantomData;
use std::ptr;

#[cfg(feature = "serde")]
use serde::de::{Deserialize, MapAccess, Visitor};

pub(crate) unsafe fn slice_insert<T>(slice: &mut [T], new: T, idx: usize) {
    let len = slice.len();
    let slice = slice.as_mut_ptr();
    ptr::copy(slice.add(idx), slice.add(idx + 1), len - idx - 1);
    ptr::write(slice.add(idx), new);
}

// From std::collections::btree::node.rs
pub(crate) unsafe fn slice_remove<T>(slice: &mut [T], idx: usize) -> T {
    // setup the value to be returned, IE give ownership to ret.
    let len = slice.len();
    let ret = ptr::read(slice.get_unchecked(idx));
    let slice = slice.as_mut_ptr();
    ptr::copy(slice.add(idx + 1), slice.add(idx), len - idx - 1);
    ret
}

pub(crate) unsafe fn slice_merge<T>(dst: &mut [T], start_idx: usize, src: &mut [T], count: usize) {
    let dst_ptr = dst.as_mut_ptr().add(start_idx);
    let src_ptr = src.as_ptr();

    ptr::copy_nonoverlapping(src_ptr, dst_ptr, count);
}

pub(crate) unsafe fn slice_move<T>(
    dst: &mut [T],
    dst_start_idx: usize,
    src: &mut [T],
    src_start_idx: usize,
    count: usize,
) {
    let dst_ptr = dst.as_mut_ptr().add(dst_start_idx);
    let src_ptr = src.as_ptr().add(src_start_idx);

    ptr::copy_nonoverlapping(src_ptr, dst_ptr, count);
}

/*
pub(crate) unsafe fn slice_slide_and_drop<T>(
    slice: &mut [MaybeUninit<T>],
    idx: usize,
    count: usize,
) {
    // drop everything up to and including idx
    for item in slice.iter_mut().take(idx + 1) {
        // These are dropped here ...?
        ptr::drop_in_place(item.as_mut_ptr());
    }
    // now move everything down.
    ptr::copy(slice.as_ptr().add(idx + 1), slice.as_mut_ptr(), count);
}

pub(crate) unsafe fn slice_slide<T>(slice: &mut [T], idx: usize, count: usize) {
    // now move everything down.
    ptr::copy(slice.as_ptr().add(idx + 1), slice.as_mut_ptr(), count);
}
*/

pub(crate) fn slice_search_linear<K, Q>(slice: &[K], k: &Q) -> Result<usize, usize>
where
    K: Borrow<Q>,
    Q: Ord + ?Sized,
{
    for (idx, nk) in slice.iter().enumerate() {
        let r = k.cmp(nk.borrow());
        match r {
            Ordering::Greater => {}
            Ordering::Equal => return Ok(idx),
            Ordering::Less => return Err(idx),
        }
    }
    Err(slice.len())
}

#[cfg(feature = "serde")]
pub struct MapCollector<T, K, V>(PhantomData<(T, K, V)>);

#[cfg(feature = "serde")]
impl<T, K, V> MapCollector<T, K, V> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

#[cfg(feature = "serde")]
impl<'de, T, K, V> Visitor<'de> for MapCollector<T, K, V>
where
    T: FromIterator<(K, V)>,
    K: Deserialize<'de>,
    V: Deserialize<'de>,
{
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        iter::from_fn(|| access.next_entry().transpose()).collect()
    }
}

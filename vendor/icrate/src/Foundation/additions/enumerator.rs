//! Utilities for the `NSEnumerator` class.
#![cfg(feature = "Foundation_NSEnumerator")]
use objc2::mutability::{IsIdCloneable, IsMutable};

use super::iter;
use crate::Foundation::NSEnumerator;
use crate::common::*;

impl<T: Message> Iterator for NSEnumerator<T> {
    type Item = Id<T>;

    #[inline]
    fn next(&mut self) -> Option<Id<T>> {
        self.nextObject()
    }
}

unsafe impl<T: Message> iter::FastEnumerationHelper for NSEnumerator<T> {
    type Item = T;

    #[inline]
    fn maybe_len(&self) -> Option<usize> {
        None
    }
}

/// A consuming iterator over the items of a `NSEnumerator`.
#[derive(Debug)]
pub struct IntoIter<T: Message>(iter::IntoIter<NSEnumerator<T>>);

__impl_iter! {
    impl<'a, T: Message> Iterator<Item = Id<T>> for IntoIter<T> { ... }
}

impl<T: Message> objc2::rc::IdIntoIterator for NSEnumerator<T> {
    type Item = Id<T>;
    type IntoIter = IntoIter<T>;

    #[inline]
    fn id_into_iter(this: Id<Self>) -> Self::IntoIter {
        IntoIter(super::iter::IntoIter::new(this))
    }
}

// Fast enumeration advances the same internal cursor as `nextObject()`;
// see <https://developer.apple.com/documentation/foundation/nseumerator>.
// The helper iterators above rely on that guarantee.

#[cfg(all(
    test,
    feature = "Foundation_NSArray",
    feature = "Foundation_NSString",
    feature = "Foundation_NSEnumerator"
))]
mod tests {
    use super::*;
    use crate::Foundation::{NSArray, NSString};
    use objc2::rc::autoreleasepool;
    use std::borrow::ToOwned;
    use std::vec;
    use std::vec::Vec;

    #[test]
    fn next_yields_same_sequence_as_array_iteration() {
        autoreleasepool(|pool| {
            let array = NSArray::from_vec(vec![
                NSString::from_str("first"),
                NSString::from_str("second"),
                NSString::from_str("third"),
            ]);
            let mut enumerator = unsafe { array.objectEnumerator() };
            let mut values = Vec::new();
            while let Some(item) = enumerator.as_mut().next() {
                values.push(item.as_str(pool).to_owned());
            }
            assert_eq!(values, ["first", "second", "third"]);
        });
    }
}

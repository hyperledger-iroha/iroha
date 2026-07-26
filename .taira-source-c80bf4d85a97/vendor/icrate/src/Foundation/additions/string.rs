#![cfg(feature = "Foundation_NSString")]
use core::cmp;
use core::ffi::c_void;
use core::fmt;
use core::panic::RefUnwindSafe;
use core::panic::UnwindSafe;
use core::ptr::NonNull;
#[cfg(feature = "apple")]
use core::slice;
use core::str;
use std::boxed::Box;
use std::ops::Deref;
use std::vec::Vec;
#[cfg(feature = "Foundation_NSMutableString")]
use std::ops::AddAssign;
#[cfg(feature = "apple")]
use std::ffi::CStr;
#[cfg(feature = "apple")]
use std::os::raw::c_char;

use objc2::msg_send_id;
use objc2::rc::{AutoreleasePool, Id, autoreleasepool_leaking};
use objc2::runtime::__nsstring::{UTF8_ENCODING, nsstring_len, nsstring_to_str};

#[cfg(feature = "Foundation_NSMutableString")]
use crate::Foundation::NSMutableString;
use crate::Foundation::{self, NSCopying, NSString};
use crate::common::*;

// SAFETY: `NSString` is immutable and `NSMutableString` can only be mutated
// from `&mut` methods.
unsafe impl Sync for NSString {}
unsafe impl Send for NSString {}

/// `NSString` backed by caller-provided UTF-8 bytes.
///
/// Instances created through [`NSString::from_boxed_utf8_no_copy`] or
/// [`NSString::from_vec_utf8_no_copy`] borrow the supplied UTF-8 storage
/// without copying. The bytes remain owned by this wrapper to ensure the
/// Objective-C string always points to valid memory.
#[derive(Debug)]
pub struct NSStringOwnedUtf8 {
    string: Id<NSString>,
    storage: Box<[u8]>,
}

impl NSStringOwnedUtf8 {
    /// Access the underlying UTF-8 bytes.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.storage
    }

    /// Borrow the underlying Objective-C string.
    #[inline]
    pub fn as_nsstring(&self) -> &NSString {
        &self.string
    }

    /// Obtain an owned `NSString` by copying the receiver.
    #[inline]
    pub fn into_owned_string(self) -> Id<NSString> {
        let copy = self.string.copy();
        drop(self);
        copy
    }
}

impl Deref for NSStringOwnedUtf8 {
    type Target = NSString;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.string
    }
}

impl AsRef<NSString> for NSStringOwnedUtf8 {
    #[inline]
    fn as_ref(&self) -> &NSString {
        self.as_nsstring()
    }
}

impl AsRef<[u8]> for NSStringOwnedUtf8 {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

// Even if an exception occurs inside a string method, the state of the string
// (should) still be perfectly safe to access.
impl UnwindSafe for NSString {}
impl RefUnwindSafe for NSString {}

impl NSString {
    /// The number of UTF-8 code units in `self`.
    #[doc(alias = "lengthOfBytesUsingEncoding")]
    #[doc(alias = "lengthOfBytesUsingEncoding:")]
    pub fn len(&self) -> usize {
        // SAFETY: This is an instance of `NSString`
        unsafe { nsstring_len(self) }
    }

    /// The number of UTF-16 code units in the string.
    ///
    /// See also [`NSString::len`].
    #[doc(alias = "length")]
    pub fn len_utf16(&self) -> usize {
        self.length()
    }

    pub fn is_empty(&self) -> bool {
        // `lengthOfBytesUsingEncoding:` may return 0 on conversion failures.
        // `length` stays accurate regardless of internal storage, so rely on that.
        self.len_utf16() == 0
    }

    /// Get the [`str`](`prim@str`) representation of this string if it can be
    /// done efficiently.
    ///
    /// Returns [`None`] if the internal storage does not allow this to be
    /// done efficiently. Use [`NSString::as_str`] or `NSString::to_string`
    /// if performance is not an issue.
    #[doc(alias = "CFStringGetCStringPtr")]
    #[allow(unused)]
    #[cfg(feature = "apple")]
    fn as_str_wip(&self) -> Option<&str> {
        type CFStringEncoding = u32;
        #[allow(non_upper_case_globals)]
        // https://developer.apple.com/documentation/corefoundation/cfstringbuiltinencodings/kcfstringencodingutf8?language=objc
        const kCFStringEncodingUTF8: CFStringEncoding = 0x08000100;
        extern "C" {
            // https://developer.apple.com/documentation/corefoundation/1542133-cfstringgetcstringptr?language=objc
            fn CFStringGetCStringPtr(s: &NSString, encoding: CFStringEncoding) -> *const c_char;
        }
        let bytes = unsafe { CFStringGetCStringPtr(self, kCFStringEncodingUTF8) };
        let len = self.len();
        NonNull::new(bytes as *mut u8).and_then(|bytes| {
            // SAFETY: Pointer originates from CFString and is valid for `len` bytes.
            let bytes: &[u8] = unsafe { slice::from_raw_parts(bytes.as_ptr(), len) };
            str::from_utf8(bytes).ok()
        })
    }

    /// Get an [UTF-16] string slice if it can be done efficiently.
    ///
    /// Returns [`None`] if the internal storage of `self` does not allow this
    /// to be returned efficiently.
    ///
    /// See [`as_str`](Self::as_str) for the UTF-8 equivalent.
    ///
    /// [UTF-16]: https://en.wikipedia.org/wiki/UTF-16
    #[allow(unused)]
    #[cfg(feature = "apple")]
    fn as_utf16(&self) -> Option<&[u16]> {
        extern "C" {
            // https://developer.apple.com/documentation/corefoundation/1542939-cfstringgetcharactersptr?language=objc
            fn CFStringGetCharactersPtr(s: &NSString) -> *const u16;
        }
        let ptr = unsafe { CFStringGetCharactersPtr(self) };
        NonNull::new(ptr as *mut u16).map(|ptr| {
            let len = self.len_utf16();
            // SAFETY: Pointer originates from CFString and is valid for `len` u16 values.
            unsafe { slice::from_raw_parts(ptr.as_ptr(), len) }
        })
    }

    /// Get a UTF-8 [`str`] view of this `NSString`.
    ///
    /// The returned slice borrows the storage owned by Objective-C and is only
    /// valid for as long as the provided autorelease `pool` remains active. Use
    /// [`NSString::from_str`] if an owned Rust string is required.
    #[doc(alias = "UTF8String")]
    pub fn as_str<'r, 's: 'r, 'p: 'r>(&'s self, pool: AutoreleasePool<'p>) -> &'r str {
        // SAFETY: This is an instance of `NSString`
        unsafe { nsstring_to_str(self, pool) }
    }

    /// Get a view of this `NSString` as a C string that retains the trailing NUL byte.
    ///
    /// The returned [`CStr`] borrows the Objective-C storage and is valid for as
    /// long as the provided autorelease `pool` remains active. Callers that need
    /// an owned buffer should copy the bytes (e.g., `to_bytes_with_nul().to_vec()`).
    #[cfg(feature = "apple")]
    pub fn as_c_str<'r, 's: 'r, 'p: 'r>(&'s self, _pool: AutoreleasePool<'p>) -> &'r CStr {
        let ptr = self.UTF8String();
        if ptr.is_null() {
            return CStr::from_bytes_with_nul(b"\0").unwrap();
        }
        unsafe { CStr::from_ptr(ptr) }
    }

    /// Creates an immutable `NSString` by copying the given string slice.
    ///
    /// Prefer using the [`ns_string!`] macro when possible.
    ///
    /// [`ns_string!`]: crate::ns_string
    #[doc(alias = "initWithBytes")]
    #[doc(alias = "initWithBytes:length:encoding:")]
    #[allow(clippy::should_implement_trait)] // Not really sure of a better name
   pub fn from_str(string: &str) -> Id<Self> {
        unsafe { init_with_str(Self::alloc(), string) }
    }

    /// Construct an `NSString` that borrows the provided UTF-8 buffer without copying.
    ///
    /// The returned wrapper keeps ownership of the bytes and guarantees that
    /// they remain alive for as long as the Objective-C string relies on them.
    /// If the bytes cannot be decoded with UTF-8, `None` is returned and the
    /// buffer is dropped.
    #[doc(alias = "initWithBytesNoCopy:length:encoding:freeWhenDone:")]
    pub fn from_boxed_utf8_no_copy(mut bytes: Box<[u8]>) -> Option<NSStringOwnedUtf8> {
        let len = bytes.len();
        let ptr = NonNull::new(bytes.as_mut_ptr())?;
        let string = unsafe {
            Self::initWithBytesNoCopy_length_encoding_freeWhenDone(
                Self::alloc(),
                ptr.cast(),
                len as NSUInteger,
                UTF8_ENCODING,
                false,
            )?
        };
        Some(NSStringOwnedUtf8 {
            string,
            storage: bytes,
        })
    }

    /// Construct an `NSString` that borrows UTF-8 bytes from a vector without copying.
    ///
    /// Internally this forwards to [`NSString::from_boxed_utf8_no_copy`].
    pub fn from_vec_utf8_no_copy(bytes: Vec<u8>) -> Option<NSStringOwnedUtf8> {
        Self::from_boxed_utf8_no_copy(bytes.into_boxed_slice())
    }
}

#[cfg(feature = "Foundation_NSMutableString")]
impl NSMutableString {
    /// Creates a new [`NSMutableString`] by copying the given string slice.
    #[doc(alias = "initWithBytes:length:encoding:")]
    #[allow(clippy::should_implement_trait)] // Not really sure of a better name
    pub fn from_str(string: &str) -> Id<Self> {
        unsafe { init_with_str(Self::alloc(), string) }
    }
}

#[cfg(all(test, feature = "Foundation_NSString", feature = "apple"))]
mod tests {
    use super::*;
    use objc2::rc::autoreleasepool;
    use std::ffi::CStr;
    use std::vec;

    #[test]
    fn as_str_wip_agrees_with_as_str() {
        let sample = NSString::from_str("Hello, world!");
        let wip = sample.as_str_wip().expect("UTF-8 view");
        assert_eq!(wip, "Hello, world!");
        autoreleasepool(|pool| {
            assert_eq!(wip, sample.as_str(pool));
        });
    }

    #[test]
    fn as_utf16_returns_expected_units() {
        let sample = NSString::from_str("Grinning 😀");
        let utf16 = sample.as_utf16().expect("UTF-16 view");
        let rust_utf16: Vec<u16> = "Grinning 😀".encode_utf16().collect();
        assert_eq!(utf16, rust_utf16.as_slice());
    }

    #[test]
    fn as_c_str_includes_trailing_nul() {
        let sample = NSString::from_str("hello");
        autoreleasepool(|pool| {
            let c_str = sample.as_c_str(pool);
            assert_eq!(c_str.to_bytes_with_nul(), b"hello\0");
        });

        let unicode = NSString::from_str("こんにちは");
        autoreleasepool(|pool| {
            let c_str = unicode.as_c_str(pool);
            let mut expected = "こんにちは".as_bytes().to_vec();
            expected.push(0);
            assert_eq!(c_str.to_bytes_with_nul(), expected.as_slice());
        });

        let empty = NSString::from_str("");
        autoreleasepool(|pool| {
            let c_str = empty.as_c_str(pool);
            assert_eq!(
                c_str.to_bytes_with_nul(),
                CStr::from_bytes_with_nul(b"\0")
                    .unwrap()
                    .to_bytes_with_nul()
            );
        });
    }

    #[test]
    fn from_boxed_utf8_no_copy_reuses_storage() {
        let bytes = b"hello world".to_vec().into_boxed_slice();
        let expected_ptr = bytes.as_ptr();
        let owned = NSString::from_boxed_utf8_no_copy(bytes).expect("no-copy string");
        autoreleasepool(|pool| {
            assert_eq!(owned.as_str(pool), "hello world");
        });
        let actual_ptr = owned.UTF8String() as *const u8;
        assert_eq!(actual_ptr, expected_ptr);
        assert_eq!(owned.as_bytes(), b"hello world");
    }

    #[test]
    fn from_vec_utf8_no_copy_invalid_returns_none() {
        let owned = NSString::from_vec_utf8_no_copy(vec![0xFF, 0xFF]);
        assert!(owned.is_none());
    }
}

unsafe fn init_with_str<T: Message>(obj: Option<Allocated<T>>, string: &str) -> Id<T> {
    let bytes: *const c_void = string.as_ptr().cast();
    // We use `msg_send_id` instead of the generated method from `icrate`,
    // since that assumes the encoding is `usize`, whereas GNUStep assumes
    // `i32`.
    unsafe {
        msg_send_id![
            obj,
            initWithBytes: bytes,
            length: string.len(),
            encoding: UTF8_ENCODING,
        ]
    }
}

#[cfg(feature = "Foundation_NSMutableString")]
impl PartialEq<NSString> for NSMutableString {
    #[inline]
    fn eq(&self, other: &NSString) -> bool {
        PartialEq::eq(&**self, other)
    }
}

#[cfg(feature = "Foundation_NSMutableString")]
impl PartialEq<NSMutableString> for NSString {
    #[inline]
    fn eq(&self, other: &NSMutableString) -> bool {
        PartialEq::eq(self, &**other)
    }
}

impl PartialOrd for NSString {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NSString {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.compare(other).into()
    }
}

#[cfg(feature = "Foundation_NSMutableString")]
impl PartialOrd for NSMutableString {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        PartialOrd::partial_cmp(&**self, &**other)
    }
}

#[cfg(feature = "Foundation_NSMutableString")]
impl PartialOrd<NSString> for NSMutableString {
    #[inline]
    fn partial_cmp(&self, other: &NSString) -> Option<cmp::Ordering> {
        PartialOrd::partial_cmp(&**self, other)
    }
}

#[cfg(feature = "Foundation_NSMutableString")]
impl PartialOrd<NSMutableString> for NSString {
    #[inline]
    fn partial_cmp(&self, other: &NSMutableString) -> Option<cmp::Ordering> {
        PartialOrd::partial_cmp(self, &**other)
    }
}

#[cfg(feature = "Foundation_NSMutableString")]
impl Ord for NSMutableString {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        Ord::cmp(&**self, &**other)
    }
}

impl PartialEq<str> for NSString {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        autoreleasepool_leaking(|pool| self.as_str(pool) == other)
    }
}

impl PartialEq<NSString> for str {
    #[inline]
    fn eq(&self, other: &NSString) -> bool {
        other == self
    }
}

#[cfg(feature = "Foundation_NSMutableString")]
impl PartialEq<str> for NSMutableString {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        PartialEq::eq(&**self, other)
    }
}

#[cfg(feature = "Foundation_NSMutableString")]
impl PartialEq<NSMutableString> for str {
    #[inline]
    fn eq(&self, other: &NSMutableString) -> bool {
        other == self
    }
}

impl PartialOrd<str> for NSString {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        autoreleasepool_leaking(|pool| Some(self.as_str(pool).cmp(other)))
    }
}

impl PartialOrd<NSString> for str {
    #[inline]
    fn partial_cmp(&self, other: &NSString) -> Option<cmp::Ordering> {
        other.partial_cmp(self).map(|ord| ord.reverse())
    }
}

#[cfg(feature = "Foundation_NSMutableString")]
impl PartialOrd<str> for NSMutableString {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        PartialOrd::partial_cmp(&**self, other)
    }
}

#[cfg(feature = "Foundation_NSMutableString")]
impl PartialOrd<NSMutableString> for str {
    #[inline]
    fn partial_cmp(&self, other: &NSMutableString) -> Option<cmp::Ordering> {
        other.partial_cmp(self).map(|ord| ord.reverse())
    }
}

#[cfg(feature = "Foundation_NSMutableString")]
impl AddAssign<&NSString> for NSMutableString {
    #[inline]
    fn add_assign(&mut self, other: &NSString) {
        self.appendString(other)
    }
}

impl fmt::Display for NSString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        autoreleasepool_leaking(|pool| fmt::Display::fmt(self.as_str(pool), f))
    }
}

#[cfg(feature = "Foundation_NSMutableString")]
impl fmt::Display for NSMutableString {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl fmt::Debug for NSString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        autoreleasepool_leaking(|pool| fmt::Debug::fmt(self.as_str(pool), f))
    }
}

#[cfg(feature = "Foundation_NSMutableString")]
impl fmt::Write for NSMutableString {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let nsstring = NSString::from_str(s);
        self.appendString(&nsstring);
        Ok(())
    }
}

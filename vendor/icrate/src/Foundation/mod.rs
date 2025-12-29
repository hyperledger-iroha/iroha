//! Bindings to the `Foundation` framework.
//!
//! This is the [`std`] equivalent for Objective-C, containing essential data
//! types, collections, and operating-system services.
//!
//! See [Apple's documentation](https://developer.apple.com/documentation/foundation?language=objc).
//!
//!
//! ## Philosophy
//!
//! The `Foundation` framework is _huge_! If we aspired to map every API it
//! exposes (a lot of it is just helper methods to make Objective-C more
//! ergonomic), this library would never be finished. Instead, our focus lies
//! on conversion methods, to allow easily using them from Rust.
//!
//! If you find some API that an object doesn't expose (but should), we gladly
//! accept [pull requests]. If it is something that is out of scope, these
//! objects implement the [`Message`] trait, so you can always just manually
//! call a method on them using the [`msg_send!`] family of macros.
//!
//! [pull requests]: https://github.com/madsmtm/objc2/pulls
//! [`Message`]: crate::objc2::Message
//! [`msg_send!`]: crate::objc2::msg_send
//!
//!
//! # Use of `Deref`
//!
//! `icrate` uses the [`Deref`] trait in a bit special way: All objects deref
//! to their superclasses. For example, `NSMutableArray` derefs to `NSArray`,
//! which in turn derefs to `NSObject`.
//!
//! Note that this is explicitly recommended against in [the
//! documentation][`Deref`] and [the Rust Design patterns
//! book][anti-pattern-deref] (see those links for details).
//!
//! Due to Objective-C objects only ever being accessible behind pointers in
//! the first place, the problems stated there are less severe, and having the
//! implementation just means that everything is much nicer when you actually
//! want to use the objects!
//!
//! All objects also implement [`AsRef`] and [`AsMut`] to their superclass,
//! and can be used in [`Id::into_super`], so if you favour explicit
//! conversion, that is a possibility too.
//!
//! [`Deref`]: std::ops::Deref
//! [`ClassType`]: crate::objc2::ClassType
//! [anti-pattern-deref]: https://rust-unofficial.github.io/patterns/anti_patterns/deref.html
//! [`Id::into_super`]: objc2::rc::Id::into_super
//! [`objc2::rc::autoreleasepool`]: objc2::rc::autoreleasepool
//! [`objc2::mutability::Mutable`]: objc2::mutability::Mutable
//! [`objc2::mutability::Immutable`]: objc2::mutability::Immutable
//!
//!
//! # Ownership
//!
//! Objective-C objects are reference-counted. `icrate` models this by wrapping
//! owned instances in [`objc2::rc::Id`]. Constructors that follow Cocoa’s
//! “create/copy” rules return `Id<Self>` so the caller owns a strong reference.
//! Borrowed views of existing objects are exposed as `&T` or `Option<&T>` and
//! never outlive the original owner.
//!
//! When you hand an Objective-C object to another API, retain semantics are the
//! same as in native code: handing out an `&T` keeps ownership with the callee,
//! whereas passing `Id<T>` transfers the strong reference. If an API requires
//! an autoreleased temporary, wrap the call in [`objc2::rc::autoreleasepool`].
//! Collections such as `NSArray` and `NSDictionary` automatically retain their
//! elements when you insert `Id<T>` or `&T` thanks to the helper utilities in
//! `Foundation::additions`.
//!
//! Mutability follows Cocoa conventions. Types that inherit from
//! `NSMutable*` expose methods requiring `&mut T` and are marked with
//! [`objc2::mutability::Mutable`] so the borrow checker enforces exclusivity.
//! Immutable variants implement [`objc2::mutability::Immutable`] and are safe
//! to share across threads whenever Apple’s documentation guarantees it.
//!
//!
//! # Rust vs. Objective-C types
//!
//! | Objective-C | (approximately) equivalent Rust |
//! | --- | --- |
//! | `NSData*` | `Arc<[u8]>` |
//! | `NSMutableData*` | `Vec<u8>` |
//! | `NSString*` | `Arc<str>` |
//! | `NSMutableString*` | `String` |
//! | `NSValue*` | `Arc<dyn Any>` |
//! | `NSNumber*` | `Arc<enum { I8(i8), U8(u8), I16(i16), U16(u16), I32(i32), U32(u32), I64(i64), U64(u64), F32(f32), F64(f64), CLong(ffi::c_long), CULong(ffi::c_ulong) }>` |
//! | `NSError*` | `Arc<dyn Error + Send + Sync>` |
//! | `NSException*` | `Arc<dyn Error + Send + Sync>` |
//! | `NSRange` | `ops::Range<usize>` |
//! | `NSComparisonResult` | `cmp::Ordering` |
//! | `NSArray<T>*` | `Arc<[T]>` |
//! | `NSMutableArray<T>*` | `Vec<T>` |
//! | `NSDictionary<K, V>*` | `Arc<HashMap<K, V>>` |
//! | `NSMutableDictionary<K, V>*` | `HashMap<K, V>` |
//! | `NSEnumerator<T>*` | `Box<dyn Iterator<T>>` |
//! | `NSCopying*` | `Box<dyn Clone>` |
#![allow(unused_imports)]

#[doc(hidden)]
pub mod __macro_helpers;
mod additions;
mod fixes;
#[path = "../generated/Foundation/mod.rs"]
mod generated;
mod macros;

pub use self::additions::*;
pub use self::fixes::*;
pub use self::generated::*;

// Available under Foundation, so makes sense here as well:
// https://developer.apple.com/documentation/foundation/numbers_data_and_basic_values?language=objc
pub use objc2::ffi::{NSInteger, NSUInteger};

// Special types that are stored in `objc2`, but really belong here
#[doc(inline)]
#[cfg(feature = "Foundation_NSProxy")]
pub use objc2::runtime::__NSProxy as NSProxy;
pub use objc2::runtime::{NSObject, NSObjectProtocol, NSZone};
#[doc(inline)]
pub use objc2::runtime::{
    __Copyhelper as Copyhelper, __NSCopying as NSCopying, __NSMutableCopying as NSMutableCopying,
};

// Link to the correct framework
#[cfg_attr(feature = "apple", link(name = "Foundation", kind = "framework"))]
#[cfg_attr(feature = "gnustep-1-7", link(name = "gnustep-base", kind = "dylib"))]
extern "C" {}

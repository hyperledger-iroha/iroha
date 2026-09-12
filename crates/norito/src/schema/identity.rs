//! Explicit schema identities composed independently of Rust source locations.
//!
//! Typed readers and writers share this single frame identity contract.
//! Payload codecs remain independent; schema inspection never selects a frame hash.

use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, LinkedList, VecDeque},
    marker::PhantomData,
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
    rc::Rc,
    sync::Arc,
};

/// A declared nominal identity and its single root-frame projection.
///
/// Generic parents compose [`Self::nominal_name`], never [`Self::frame_name`].
/// For example, borrowed strings share the `String` root frame while a vector
/// of borrowed strings retains its distinct nominal element identity. Names
/// are protocol declarations: they must not be inferred from `type_name`,
/// `module_path`, or an alias registry.
///
/// This trait has no serialization bound, so phantom markers and typed hashes
/// can declare identities without making the marker serializable.
pub trait NoritoSchema {
    /// Canonical nominal identity, including ordered type, const and erased lifetime slots.
    fn nominal_name() -> String;

    /// The single identity advertised by a frame containing this root type.
    fn frame_name() -> String {
        Self::nominal_name()
    }
}

/// Compute the fixed domain-separated digest of a declared root-frame identity.
///
/// This function is not an overridable part of the identity declaration.
pub fn frame_hash<T: NoritoSchema + ?Sized>() -> [u8; 16] {
    crate::core::schema_hash_for_name(&T::frame_name())
}

/// Compose a named constructor with ordered nominal type or const arguments.
///
/// Each argument must already be a canonical identity or a Rust primitive
/// const literal. Empty argument lists leave the constructor unchanged.
pub fn generic_name(constructor: &str, arguments: &[String]) -> String {
    if arguments.is_empty() {
        constructor.to_owned()
    } else {
        format!("{constructor}<{}>", arguments.join(", "))
    }
}

macro_rules! nominal {
    ($($ty:ty => $name:literal),+ $(,)?) => {$(
        impl NoritoSchema for $ty {
            fn nominal_name() -> String { $name.to_owned() }
        }
    )+};
}
nominal! {
    () => "()", bool => "bool", char => "char",
    u8 => "u8", u16 => "u16", u32 => "u32", u64 => "u64", u128 => "u128", usize => "usize",
    i8 => "i8", i16 => "i16", i32 => "i32", i64 => "i64", i128 => "i128", isize => "isize",
    f32 => "f32", f64 => "f64", str => "str", String => "alloc::string::String",
    NonZeroU16 => "core::num::nonzero::NonZero<u16>",
    NonZeroU32 => "core::num::nonzero::NonZero<u32>",
    NonZeroU64 => "core::num::nonzero::NonZero<u64>",
}

macro_rules! unary {
    ($($ty:ident => $name:literal),+ $(,)?) => {$(
        impl<T: NoritoSchema> NoritoSchema for $ty<T> {
            fn nominal_name() -> String { generic_name($name, &[T::nominal_name()]) }
        }
    )+};
}
unary! {
    Box => "alloc::boxed::Box", Rc => "alloc::rc::Rc", Arc => "alloc::sync::Arc",
    Cell => "core::cell::Cell", RefCell => "core::cell::RefCell",
    Option => "core::option::Option", Vec => "alloc::vec::Vec",
    VecDeque => "alloc::collections::vec_deque::VecDeque",
    LinkedList => "alloc::collections::linked_list::LinkedList",
    BinaryHeap => "alloc::collections::binary_heap::BinaryHeap",
    BTreeSet => "alloc::collections::btree::set::BTreeSet",
    HashSet => "std::collections::hash::set::HashSet",
}

impl<T: NoritoSchema + ?Sized> NoritoSchema for PhantomData<T> {
    fn nominal_name() -> String {
        generic_name("core::marker::PhantomData", &[T::nominal_name()])
    }
}

macro_rules! binary {
    ($($ty:ident => $name:literal),+ $(,)?) => {$(
        impl<T: NoritoSchema, U: NoritoSchema> NoritoSchema for $ty<T, U> {
            fn nominal_name() -> String {
                generic_name($name, &[T::nominal_name(), U::nominal_name()])
            }
        }
    )+};
}
binary! {
    Result => "core::result::Result",
    BTreeMap => "alloc::collections::btree::map::BTreeMap",
    HashMap => "std::collections::hash::map::HashMap",
}

impl<T: NoritoSchema, const N: usize> NoritoSchema for [T; N] {
    fn nominal_name() -> String {
        format!("[{}; {N}]", T::nominal_name())
    }
}

impl<T: NoritoSchema> NoritoSchema for &T {
    fn nominal_name() -> String {
        format!("&{}", T::nominal_name())
    }
}

macro_rules! string_projection {
    ($($ty:ty => $name:literal),+ $(,)?) => {$(
        impl NoritoSchema for $ty {
            fn nominal_name() -> String { $name.to_owned() }
            fn frame_name() -> String { String::nominal_name() }
        }
    )+};
}
string_projection! {
    &str => "&str",
    Cow<'_, str> => "alloc::borrow::Cow<'_, str>",
    Box<str> => "alloc::boxed::Box<str>",
}

macro_rules! tuple {
    ($($ty:ident),+) => {
        impl<$($ty: NoritoSchema),+> NoritoSchema for ($($ty,)+) {
            fn nominal_name() -> String {
                format!("({})", [$($ty::nominal_name()),+].join(", "))
            }
        }
    };
}
tuple!(A, B);
tuple!(A, B, C);
tuple!(A, B, C, D);
tuple!(A, B, C, D, E);
tuple!(A, B, C, D, E, F);
tuple!(A, B, C, D, E, F, G);
tuple!(A, B, C, D, E, F, G, H);
tuple!(A, B, C, D, E, F, G, H, I);
tuple!(A, B, C, D, E, F, G, H, I, J);
tuple!(A, B, C, D, E, F, G, H, I, J, K);
tuple!(A, B, C, D, E, F, G, H, I, J, K, L);

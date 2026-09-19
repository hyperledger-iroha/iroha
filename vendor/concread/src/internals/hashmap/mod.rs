//! HashMap - A concurrently readable HashMap
//!
//! This is a specialisation of the `BptreeMap`, allowing a concurrently readable
//! HashMap. Unlike a traditional hashmap it does *not* have `O(1)` lookup, as it
//! internally uses a tree-like structure to store a series of buckets. However
//! if you do not need key-ordering, due to the storage of the hashes as `u64`
//! the operations in the tree to seek the bucket is much faster than the use of
//! the same key in the `BptreeMap`.
//!
//! For more details. see the [BptreeMap](crate::bptree::BptreeMap)
//!
//! This structure is very different to the `im` crate. The `im` crate is
//! sync + send over individual operations. This means that multiple writes can
//! be interleaved atomically and safely, and the readers always see the latest
//! data. While this is potentially useful to a set of problems, transactional
//! structures are suited to problems where readers have to maintain consistent
//! data views for a duration of time, cpu cache friendly behaviours and
//! database like transaction properties (ACID).

#[macro_use]
mod macros;
pub mod cursor;
pub mod iter;
pub(crate) mod node;
mod simd;
mod states;

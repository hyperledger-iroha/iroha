//! HashTrie - A concurrently readable HashTrie
//!
//! This is similar to `HashMap`, however is based on a suffix trie which
//! is append only / additive only. As a result, the more you add, the more
//! space this will take up. The only way to "remove" an item would be to swap
//! it's value with a "None". The trie won't shrink, but node size requirements
//! are low. For a trie with 4,294,967,295 items, only ~40MB is required. For
//! 1,000,000 items only ~12KB is required.
//!
//! If in doubt, you should use `HashMap` instead 🥰

// mod states;
// mod node;
pub mod cursor;
pub mod iter;

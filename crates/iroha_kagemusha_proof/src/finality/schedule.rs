//! Source-bound decoding of the validator schedule in an authenticated result.
//!
//! The internal byte-tape root is a recursive witness commitment, never native
//! authority. Its complete consecutive hash scan must close to the result digest
//! certified by the predecessor-authenticated native quorum. Parser cursor and
//! container bounds, tape root and length must share that recursive context.

pub mod authorization;
pub mod decode;
pub mod epoch;
pub mod graph;
pub mod tape;

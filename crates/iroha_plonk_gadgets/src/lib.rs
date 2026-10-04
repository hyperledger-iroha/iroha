//! Chips for the Iroha-native PIPA-v1 `PLONKish` engine ([`iroha_plonk`],
//! `specs/plonk_ipa_v1.md`) on the Pasta fields of [`iroha_pasta`].
//!
//! # Contents
//!
//! Stage GADGETS (task T17):
//!
//! - [`poseidon`]: the RP57 width-3 Pow5 permutation lane (37 rows and 148
//!   cells per permutation, the M8 layout) and the KAGEMUSHA
//!   domain/arity-prefixed sponge on it, bit for bit
//!   [`iroha_pasta::poseidon`] and `kagemusha_v1_poseidon::hash`;
//! - [`range`]: running-sum range checks against a `2^b`-row table, and
//!   checked `u128`/`u64` add, subtract and compare whose overflow or
//!   underflow has no satisfying assignment;
//! - [`arith`]: the glue gate (add, multiply, linear combinations,
//!   constants, booleans, select, is-zero, equality);
//! - [`statement`]: the **prototype** G1 statement encoding of the
//!   split-lineage step relations (M7), and the canonical cross-field limb
//!   encoding of spec S6. The split-lineage design awaits owner approval;
//!   nothing here is wired into a protocol path.
//! - [`cells`]: the typed cells chips exchange ([`cells::Word`],
//!   [`cells::Bit`], [`cells::Uint`]) and row cursors;
//! - [`tamper`]: the per-cell tamper harness every chip test runs.
//!
//! # Discipline
//!
//! Every chip has typed assigned cells, a native reference, shared vectors,
//! a per-cell tamper suite (each assigned advice cell, changed alone, must
//! make the strict constraint checker fail) and an inventory test pinning
//! its rows and cells per operation (`tests/`). Every gate has degree at
//! most [`MAX_GATE_DEGREE`]: with exact cosets the quotient cost scales with
//! `d - 1` (spec section 1 keeps the format cap at 9; this crate's policy is
//! 6).
//!
//! # Layout
//!
//! The `iroha_plonk` floor planner starts every region at row 0, so chips
//! address absolute rows: each chip owns its columns and a row cursor, and
//! chips on disjoint columns share rows freely. Pow5 lanes place
//! permutations at multiples of 37 rows, so lanes may share their
//! round-constant columns.
//!
//! # Determinism
//!
//! Layouts and witnesses are pure functions of the inputs: ordered
//! containers only, checked row arithmetic, no environment variables, no
//! `unsafe`. Witness generation that depends on secret values (inverses,
//! zero tests) is constant time.
#![forbid(unsafe_code)]

pub mod arith;
pub mod cells;
pub mod poseidon;
pub mod range;
pub mod statement;
pub mod tamper;

pub use arith::{GlueChip, GlueConfig};
pub use cells::{Bit, RowCursor, U64, U128, Uint, Word};
pub use poseidon::{
    AbsorbInput, Pow5Chip, Pow5Columns, Pow5Config, RoundConstantColumns, SpongeChip, SpongeConfig,
};
pub use range::{LimbBits, RunningSumChip, RunningSumConfig, UintChip};

/// The largest gate degree any chip of this crate uses.
pub const MAX_GATE_DEGREE: usize = 6;

//! Native folding of `iroha_plonk` relations: the engine of the KAGEMUSHA folding
//! lineage method (design: `output/kagemusha-nova-folding-20261106/DESIGN.md`).
//!
//! A [`FoldedRelation`] is compiled from the constraint system and fixed tables of
//! one `iroha_plonk` circuit, so the folded relation keeps that circuit's gate
//! polynomials over its fixed columns and selectors exactly. Lookups are
//! re-arithmetized as `LogUp`: round one commits the advice columns and the
//! multiplicities, the challenges `theta` and `r` follow, and round two commits
//! the reciprocal columns. Inputs of single-column lookups that share a table and a
//! row share one reciprocal cell (at most three). Copy constraints and public
//! inputs are affine relations of the witness: they hold for every fresh instance,
//! hence for every fold, and the decider checks them once.
//!
//! [`fold::fold`] folds one fresh instance into an accumulator with the
//! `ProtoGalaxy` rule (Eagen–Gabizon, ePrint 2023/1106, k = 1): the prover sends
//! the coefficients of `F(X)` and `K(X)` and no cross-term commitment; the native
//! verifier [`fold::verify_fold`] recomputes every challenge and the folded
//! instance. [`decide::decide`] is the native (non-succinct) decider used as the
//! test oracle. [`decider_v2`] is the production decider: a zero-check and a
//! shift-reduction sumcheck reduce an accumulator (committed under the monomial
//! generators) to evaluation claims at one point for one batched IPA opening.
//! [`decider`] is the relaxed-PLONK decider circuit (v1), kept as the baseline.
//! [`claims`] folds IPA generator claims `(G, u)` as committed tensor vectors.
//!
//! Every transcript of instances committed on a curve runs over that curve's base
//! field ([`transcript`]), the field of the circuit that folds them in the
//! two-curve IVC (output `IVC.md`): A circuits over `Fp` fold Q instances
//! committed on Pallas, Q circuits over `Fq` fold A instances committed on Vesta.
//! [`circuit`] verifies a fold in such a circuit, bit for bit [`fold::verify_fold`].
//!
//! Nothing in this crate is a succinct proof or a zero-knowledge argument: folds
//! are local to the wallet and leave it only through a compression proof.

#![forbid(unsafe_code)]

pub mod circuit;
pub mod claims;
pub mod commit;
pub mod decide;
pub mod decider;
pub mod decider_v2;
pub mod fold;
pub mod ivc;
pub mod logup;
pub mod program;
pub mod relation;
pub mod transcript;

pub use commit::{CommitKey, Witness};
pub use fold::{Accumulator, FoldProof, Instance};
pub use relation::FoldedRelation;

use core::fmt;

/// Errors of compilation, proving, folding and deciding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The synthesized stage carries no advice values.
    MissingWitness,
    /// Column counts, lengths or degrees do not match the relation.
    Shape(&'static str),
    /// A lookup input tuple is absent from its table (a prover input error).
    LookupInput {
        /// Lookup index in the constraint system.
        lookup: usize,
        /// Row of the missing input.
        row: usize,
    },
    /// A multi-scalar multiplication failed (memory budget or input bound).
    Msm,
    /// A folded degree exceeds the number of evaluation points this engine supports.
    Degree(usize),
    /// The decider rejected the accumulator; names the failing check.
    Rejected(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingWitness => f.write_str("the synthesized stage has no witness"),
            Self::Shape(what) => write!(f, "shape mismatch: {what}"),
            Self::LookupInput { lookup, row } => {
                write!(f, "lookup {lookup}: input at row {row} is not in its table")
            }
            Self::Msm => f.write_str("multi-scalar multiplication failed"),
            Self::Degree(degree) => write!(f, "folded degree {degree} is not supported"),
            Self::Rejected(check) => write!(f, "decider rejected: {check}"),
        }
    }
}

impl std::error::Error for Error {}

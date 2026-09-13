use std::marker::PhantomData;

use crate::arithmetic::CurveAffine;

mod prover;
mod verifier;

#[cfg(test)]
pub(in crate::plonk) use prover::{
    StoredVanishingOrdinaryOracleV1, stored_vanishing_ordinary_oracle,
};

/// A vanishing argument.
pub(crate) struct Argument<C: CurveAffine> {
    _marker: PhantomData<C>,
}

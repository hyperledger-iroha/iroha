//! Additive group over the Pasta Vesta curve (dual of Pallas).

use core::fmt;

use halo2curves::{
    ff::Field,
    group::{Group, GroupEncoding},
    pasta::{Fp, Vesta, VestaAffine},
};

use crate::{errors::Error, field::PrimeField64};

/// Group element on the Vesta curve (projective form).
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct GroupElem(Vesta);

impl fmt::Debug for GroupElem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.to_bytes();
        write!(f, "GroupElem(0x{} )", hex::encode(bytes))
    }
}

impl GroupElem {
    /// Constructs a group element from a projective point.
    pub fn new(inner: Vesta) -> Self {
        Self(inner)
    }

    /// Returns the additive identity element (point at infinity).
    pub fn identity() -> Self {
        Self(Vesta::identity())
    }

    /// Adds two group elements.
    #[allow(clippy::should_implement_trait)]
    pub fn mul(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }

    /// Negates the group element.
    pub fn inv(self) -> Result<Self, Error> {
        Ok(Self(-self.0))
    }

    /// Scales the group element by a scalar (field element).
    pub fn pow(self, exp: PrimeField64) -> Self {
        let scalar: Fp = exp.into();
        Self(self.0 * scalar)
    }

    /// Convert to canonical compressed bytes.
    pub fn to_bytes(self) -> [u8; 32] {
        let affine = VestaAffine::from(self.0);
        let repr = affine.to_bytes();
        let mut out = [0u8; 32];
        out.copy_from_slice(repr.as_ref());
        out
    }

    /// Reconstruct a group element from canonical compressed bytes.
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, Error> {
        let mut repr = <VestaAffine as GroupEncoding>::Repr::default();
        repr.as_mut().copy_from_slice(bytes);
        VestaAffine::from_bytes(&repr)
            .into_option()
            .map(|aff| Self(Vesta::from(aff)))
            .ok_or(Error::InvalidEncoding)
    }

    /// Access the underlying projective point.
    pub fn inner(&self) -> Vesta {
        self.0
    }
}

/// Derive a group element deterministically from a scalar.
pub fn from_scalar(scalar: PrimeField64) -> GroupElem {
    let mut inner: Fp = scalar.into();
    if inner.is_zero().into() {
        inner = Fp::ONE;
    }
    GroupElem(Vesta::generator() * inner)
}

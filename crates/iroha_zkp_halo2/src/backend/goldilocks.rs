//! Goldilocks (64-bit) backend with a deterministic multiplicative group.

use core::{convert::TryInto, fmt};

use crate::{
    backend::{IpaBackend, IpaGroup, IpaScalar},
    errors::Error,
    norito_types::ZkCurveId,
};

const MODULUS: u128 = 0xffff_ffff_0000_0001;
const MODULUS_U64: u64 = MODULUS as u64;

#[inline]
fn reduce_u128(mut value: u128) -> u64 {
    if value >= MODULUS {
        value -= MODULUS;
    }
    value as u64
}

/// Scalar field element over Goldilocks prime (2^64 - 2^32 + 1).
#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub struct Scalar(u64);

impl fmt::Debug for Scalar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GoldilocksScalar(0x{:016x})", self.0)
    }
}

impl Scalar {
    /// Return the additive identity element of the Goldilocks field.
    #[inline]
    pub fn zero() -> Self {
        Self(0)
    }

    /// Return the multiplicative identity element of the Goldilocks field.
    #[inline]
    pub fn one() -> Self {
        Self(1)
    }

    #[inline]
    fn reduce(value: u128) -> Self {
        Self(reduce_u128(value))
    }

    #[inline]
    fn inner(self) -> u64 {
        self.0
    }

    fn pow_internal(mut base: u128, mut exp: u128) -> u64 {
        let mut acc = 1u128;
        while exp > 0 {
            if exp & 1 == 1 {
                acc = (acc * base) % MODULUS;
            }
            base = (base * base) % MODULUS;
            exp >>= 1;
        }
        acc as u64
    }
}

impl From<u64> for Scalar {
    fn from(value: u64) -> Self {
        if value >= MODULUS_U64 {
            Self(reduce_u128(value as u128))
        } else {
            Self(value)
        }
    }
}

impl From<u32> for Scalar {
    fn from(value: u32) -> Self {
        Self(value as u64)
    }
}

impl IpaScalar for Scalar {
    fn zero() -> Self {
        Scalar::zero()
    }

    fn one() -> Self {
        Scalar::one()
    }

    fn add(self, rhs: Self) -> Self {
        Scalar::reduce((self.0 as u128) + (rhs.0 as u128))
    }

    fn sub(self, rhs: Self) -> Self {
        if self.0 >= rhs.0 {
            Self(self.0 - rhs.0)
        } else {
            Self(((self.0 as u128) + MODULUS - (rhs.0 as u128)) as u64)
        }
    }

    fn mul(self, rhs: Self) -> Self {
        Self(((self.0 as u128) * (rhs.0 as u128) % MODULUS) as u64)
    }

    fn neg(self) -> Self {
        if self.0 == 0 {
            self
        } else {
            Self(MODULUS_U64 - self.0)
        }
    }

    fn inv(self) -> Result<Self, Error> {
        if self.0 == 0 {
            return Err(Error::InversionOfZero);
        }
        Ok(Self(Scalar::pow_internal(self.0 as u128, MODULUS - 2)))
    }

    fn pow_u64(self, mut exp: u64) -> Self {
        let mut base = self.0 as u128;
        let mut acc = 1u128;
        while exp > 0 {
            if exp & 1 == 1 {
                acc = (acc * base) % MODULUS;
            }
            base = (base * base) % MODULUS;
            exp >>= 1;
        }
        Self(acc as u64)
    }

    fn pow_u128(self, mut exp: u128) -> Self {
        let mut base = self.0 as u128;
        let mut acc = 1u128;
        while exp > 0 {
            if exp & 1 == 1 {
                acc = (acc * base) % MODULUS;
            }
            base = (base * base) % MODULUS;
            exp >>= 1;
        }
        Self(acc as u64)
    }

    fn to_bytes(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[..8].copy_from_slice(&self.0.to_le_bytes());
        out
    }

    fn from_bytes(bytes: &[u8; 32]) -> Result<Self, Error> {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&bytes[..8]);
        if bytes[8..].iter().any(|&b| b != 0) {
            return Err(Error::InvalidEncoding);
        }
        let value = u64::from_le_bytes(buf);
        if value >= MODULUS_U64 {
            return Err(Error::InvalidEncoding);
        }
        Ok(Self(value))
    }

    fn from_uniform(bytes: &[u8; 64]) -> Self {
        let mut acc = 0u128;
        for chunk in bytes.chunks_exact(8).rev() {
            let limb = u64::from_le_bytes(chunk.try_into().expect("chunk")) as u128;
            acc = (acc * (1u128 << 64) + limb) % MODULUS;
        }
        let mut value = acc as u64;
        if value == 0 {
            value = 1;
        }
        Self(value)
    }
}

/// Additive group over the Goldilocks field (toy backend).
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Group(Scalar);

impl fmt::Debug for Group {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GoldilocksGroup(0x{:016x})", self.0.inner())
    }
}

impl Group {
    fn from_scalar(s: Scalar) -> Self {
        Self(s)
    }
}

impl IpaGroup for Group {
    type Scalar = Scalar;

    fn identity() -> Self {
        Self(Scalar::zero())
    }

    fn mul(self, rhs: Self) -> Self {
        Self(self.0.add(rhs.0))
    }

    fn inv(self) -> Result<Self, Error> {
        Ok(Self(self.0.neg()))
    }

    fn pow(self, exp: Scalar) -> Self {
        Self(self.0.mul(exp))
    }

    fn to_bytes(self) -> [u8; 32] {
        self.0.to_bytes()
    }

    fn from_bytes(bytes: &[u8; 32]) -> Result<Self, Error> {
        let scalar = Scalar::from_bytes(bytes)?;
        Ok(Self(scalar))
    }
}

/// Goldilocks backend marker.
#[derive(Debug, Clone, Copy, Default)]
pub struct GoldilocksBackend;

impl IpaBackend for GoldilocksBackend {
    type Scalar = Scalar;
    type Group = Group;

    const CURVE_ID: ZkCurveId = ZkCurveId::Goldilocks;

    fn group_from_scalar(scalar: Self::Scalar) -> Self::Group {
        let mut v = scalar;
        if v.inner() == 0 {
            v = Scalar::one();
        }
        Group::from_scalar(v)
    }
}

/// IPA parameter set specialised to the Goldilocks backend.
pub type Params = crate::params::Params<GoldilocksBackend>;
/// Polynomial type specialised to the Goldilocks backend.
pub type Polynomial = crate::poly::Polynomial<GoldilocksBackend>;
/// IPA proof type specialised to the Goldilocks backend.
pub type IpaProof = crate::ipa::IpaProof<GoldilocksBackend>;
/// IPA prover specialised to the Goldilocks backend.
pub type IpaProver = crate::ipa::IpaProver<GoldilocksBackend>;
/// IPA verifier specialised to the Goldilocks backend.
pub type IpaVerifier = crate::ipa::IpaVerifier<GoldilocksBackend>;

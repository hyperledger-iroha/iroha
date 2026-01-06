//! Shared curve identifier registry for account controllers and address payloads.

use iroha_crypto::Algorithm;
use thiserror::Error;

/// Stable identifier assigned to a supported signing curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurveId(u8);

impl CurveId {
    /// Identifier for Ed25519 controllers.
    pub const ED25519: Self = Self(1);
    /// Identifier for secp256k1 controllers.
    pub const SECP256K1: Self = Self(4);
    /// Identifier for BLS12-381 (normal) controllers.
    #[cfg(feature = "bls")]
    pub const BLS_NORMAL: Self = Self(3);
    /// Identifier for BLS12-381 (small) controllers.
    #[cfg(feature = "bls")]
    pub const BLS_SMALL: Self = Self(5);
    /// Identifier for ML‑DSA controllers (Dilithium).
    pub const MLDSA: Self = Self(2);
    /// Identifier for GOST R 34.10-2012 256-bit curve, param set A.
    pub const GOST_256_A: Self = Self(10);
    /// Identifier for GOST R 34.10-2012 256-bit curve, param set B.
    pub const GOST_256_B: Self = Self(11);
    /// Identifier for GOST R 34.10-2012 256-bit curve, param set C.
    pub const GOST_256_C: Self = Self(12);
    /// Identifier for GOST R 34.10-2012 512-bit curve, param set A.
    pub const GOST_512_A: Self = Self(13);
    /// Identifier for GOST R 34.10-2012 512-bit curve, param set B.
    pub const GOST_512_B: Self = Self(14);
    /// Identifier for SM2 controllers.
    pub const SM2: Self = Self(15);

    /// Convert an [`Algorithm`] into the published curve identifier.
    ///
    /// # Errors
    ///
    /// Returns [`CurveRegistryError::UnsupportedAlgorithm`] when the algorithm
    /// is not assigned a `CurveId` in the registry.
    #[allow(unreachable_patterns)]
    pub fn try_from_algorithm(algorithm: Algorithm) -> Result<Self, CurveRegistryError> {
        match algorithm {
            Algorithm::Ed25519 => Ok(Self::ED25519),
            Algorithm::Secp256k1 => Ok(Self::SECP256K1),
            #[cfg(feature = "bls")]
            Algorithm::BlsNormal => Ok(Self::BLS_NORMAL),
            #[cfg(feature = "bls")]
            Algorithm::BlsSmall => Ok(Self::BLS_SMALL),
            #[cfg(feature = "ml-dsa")]
            Algorithm::MlDsa => Ok(Self::MLDSA),
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetA => Ok(Self::GOST_256_A),
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetB => Ok(Self::GOST_256_B),
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetC => Ok(Self::GOST_256_C),
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_512ParamSetA => Ok(Self::GOST_512_A),
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_512ParamSetB => Ok(Self::GOST_512_B),
            #[cfg(feature = "sm")]
            Algorithm::Sm2 => Ok(Self::SM2),
            _ => Err(CurveRegistryError::UnsupportedAlgorithm(algorithm)),
        }
    }

    /// Export the identifier as its canonical byte.
    pub const fn as_u8(self) -> u8 {
        self.0
    }

    /// Convert the identifier back into an [`Algorithm`].
    pub fn algorithm(self) -> Algorithm {
        match self.0 {
            1 => Algorithm::Ed25519,
            4 => Algorithm::Secp256k1,
            #[cfg(feature = "bls")]
            3 => Algorithm::BlsNormal,
            #[cfg(feature = "bls")]
            5 => Algorithm::BlsSmall,
            #[cfg(feature = "ml-dsa")]
            2 => Algorithm::MlDsa,
            #[cfg(feature = "gost")]
            10 => Algorithm::Gost3410_2012_256ParamSetA,
            #[cfg(feature = "gost")]
            11 => Algorithm::Gost3410_2012_256ParamSetB,
            #[cfg(feature = "gost")]
            12 => Algorithm::Gost3410_2012_256ParamSetC,
            #[cfg(feature = "gost")]
            13 => Algorithm::Gost3410_2012_512ParamSetA,
            #[cfg(feature = "gost")]
            14 => Algorithm::Gost3410_2012_512ParamSetB,
            #[cfg(feature = "sm")]
            15 => Algorithm::Sm2,
            other => unreachable!("unsupported curve id {other}"),
        }
    }
}

impl TryFrom<u8> for CurveId {
    type Error = CurveRegistryError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::ED25519),
            4 => Ok(Self::SECP256K1),
            #[cfg(feature = "bls")]
            3 => Ok(Self::BLS_NORMAL),
            #[cfg(feature = "bls")]
            5 => Ok(Self::BLS_SMALL),
            #[cfg(feature = "ml-dsa")]
            2 => Ok(Self::MLDSA),
            #[cfg(feature = "gost")]
            10 => Ok(Self::GOST_256_A),
            #[cfg(feature = "gost")]
            11 => Ok(Self::GOST_256_B),
            #[cfg(feature = "gost")]
            12 => Ok(Self::GOST_256_C),
            #[cfg(feature = "gost")]
            13 => Ok(Self::GOST_512_A),
            #[cfg(feature = "gost")]
            14 => Ok(Self::GOST_512_B),
            #[cfg(feature = "sm")]
            15 => Ok(Self::SM2),
            other => Err(CurveRegistryError::UnknownCurveId(other)),
        }
    }
}

impl From<CurveId> for u8 {
    fn from(value: CurveId) -> Self {
        value.0
    }
}

/// Error raised when a curve identifier lookup fails.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum CurveRegistryError {
    /// The supplied signing algorithm is not assigned a curve identifier.
    #[error("unsupported curve algorithm: {0}")]
    UnsupportedAlgorithm(Algorithm),
    /// An unknown identifier was decoded from canonical bytes.
    #[error("unknown curve identifier: {0}")]
    UnknownCurveId(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "bls")]
    #[test]
    fn bls_curve_ids_round_trip() {
        let normal = CurveId::try_from_algorithm(Algorithm::BlsNormal)
            .expect("BLS normal should map to a curve id");
        let small = CurveId::try_from_algorithm(Algorithm::BlsSmall)
            .expect("BLS small should map to a curve id");

        assert_eq!(normal, CurveId::BLS_NORMAL);
        assert_eq!(small, CurveId::BLS_SMALL);
        assert_eq!(CurveId::try_from(u8::from(normal)).unwrap(), normal);
        assert_eq!(CurveId::try_from(u8::from(small)).unwrap(), small);
        assert_eq!(normal.algorithm(), Algorithm::BlsNormal);
        assert_eq!(small.algorithm(), Algorithm::BlsSmall);
    }
}

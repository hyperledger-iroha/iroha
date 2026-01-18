use core::{fmt, str::FromStr};
#[cfg(not(feature = "ffi_import"))]
use std::string::String;

use iroha_schema::{IntoSchema, TypeId};
#[cfg(not(feature = "ffi_import"))]
use norito::json::{
    self, JsonDeserialize as NoritoJsonDeserialize, JsonSerialize as NoritoJsonSerialize,
};
#[cfg(not(feature = "ffi_import"))]
use norito::{Decode, Encode, core as norito_core};

use crate::error::NoSuchAlgorithm;

/// String algorithm representation
pub const ED_25519: &str = "ed25519";
/// String algorithm representation
pub const SECP_256_K1: &str = "secp256k1";
/// String algorithm representation
pub const ML_DSA: &str = "ml-dsa";
#[cfg(feature = "gost")]
/// String algorithm representation
pub const GOST_34_10_2012_256_A: &str = "gost3410-2012-256-paramset-a";
#[cfg(feature = "gost")]
/// String algorithm representation
pub const GOST_34_10_2012_256_B: &str = "gost3410-2012-256-paramset-b";
#[cfg(feature = "gost")]
/// String algorithm representation
pub const GOST_34_10_2012_256_C: &str = "gost3410-2012-256-paramset-c";
#[cfg(feature = "gost")]
/// String algorithm representation
pub const GOST_34_10_2012_512_A: &str = "gost3410-2012-512-paramset-a";
#[cfg(feature = "gost")]
/// String algorithm representation
pub const GOST_34_10_2012_512_B: &str = "gost3410-2012-512-paramset-b";
#[cfg(feature = "bls")]
/// String algorithm representation
pub const BLS_NORMAL: &str = "bls_normal";
#[cfg(feature = "bls")]
/// String algorithm representation
pub const BLS_SMALL: &str = "bls_small";
#[cfg(feature = "sm")]
/// String algorithm representation
pub const SM2: &str = "sm2";

crate::ffi::ffi_item! {
    /// Algorithm for hashing & signing
    ///
    /// Discriminants are part of the on-wire format; keep them stable and aligned
    /// with `Algorithm::try_from` and `PublicKeyCompact` tag mappings.
    #[derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Default,
        Decode,
        Encode,
        TypeId
    )]
    #[repr(u8)]
    pub enum Algorithm {
        /// Ed25519 digital signature scheme
        #[default]
        Ed25519 = 0,
        /// ECDSA over secp256k1
        Secp256k1 = 1,
        #[cfg(feature = "bls")]
        /// BLS12-381 (normal) scheme
        BlsNormal = 2,
        #[cfg(feature = "bls")]
        /// BLS12-381 (small) scheme
        BlsSmall = 3,
        /// ML‑DSA (Dilithium) post-quantum signature scheme
        MlDsa = 4,
        #[cfg(feature = "gost")]
        /// GOST R 34.10-2012 256-bit curve, TC26 param set A
        Gost3410_2012_256ParamSetA = 5,
        #[cfg(feature = "gost")]
        /// GOST R 34.10-2012 256-bit curve, TC26 param set B
        Gost3410_2012_256ParamSetB = 6,
        #[cfg(feature = "gost")]
        /// GOST R 34.10-2012 256-bit curve, TC26 param set C
        Gost3410_2012_256ParamSetC = 7,
        #[cfg(feature = "gost")]
        /// GOST R 34.10-2012 512-bit curve, TC26 param set A
        Gost3410_2012_512ParamSetA = 8,
        #[cfg(feature = "gost")]
        /// GOST R 34.10-2012 512-bit curve, TC26 param set B
        Gost3410_2012_512ParamSetB = 9,
        #[cfg(feature = "sm")]
        /// SM2 signature scheme (GM/T 0003-2012)
        Sm2 = 10,
    }
}

impl Algorithm {
    /// Maps the algorithm to its static string representation
    pub const fn as_static_str(self) -> &'static str {
        match self {
            Self::Ed25519 => ED_25519,
            Self::Secp256k1 => SECP_256_K1,
            Self::MlDsa => ML_DSA,
            #[cfg(feature = "gost")]
            Self::Gost3410_2012_256ParamSetA => GOST_34_10_2012_256_A,
            #[cfg(feature = "gost")]
            Self::Gost3410_2012_256ParamSetB => GOST_34_10_2012_256_B,
            #[cfg(feature = "gost")]
            Self::Gost3410_2012_256ParamSetC => GOST_34_10_2012_256_C,
            #[cfg(feature = "gost")]
            Self::Gost3410_2012_512ParamSetA => GOST_34_10_2012_512_A,
            #[cfg(feature = "gost")]
            Self::Gost3410_2012_512ParamSetB => GOST_34_10_2012_512_B,
            #[cfg(feature = "bls")]
            Self::BlsNormal => BLS_NORMAL,
            #[cfg(feature = "bls")]
            Self::BlsSmall => BLS_SMALL,
            #[cfg(feature = "sm")]
            Self::Sm2 => SM2,
        }
    }
}

impl fmt::Display for Algorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_static_str())
    }
}

impl FromStr for Algorithm {
    type Err = NoSuchAlgorithm;

    fn from_str(algorithm: &str) -> Result<Self, Self::Err> {
        match algorithm {
            ED_25519 => Ok(Algorithm::Ed25519),
            SECP_256_K1 => Ok(Algorithm::Secp256k1),
            ML_DSA => Ok(Algorithm::MlDsa),
            #[cfg(feature = "gost")]
            GOST_34_10_2012_256_A => Ok(Algorithm::Gost3410_2012_256ParamSetA),
            #[cfg(feature = "gost")]
            GOST_34_10_2012_256_B => Ok(Algorithm::Gost3410_2012_256ParamSetB),
            #[cfg(feature = "gost")]
            GOST_34_10_2012_256_C => Ok(Algorithm::Gost3410_2012_256ParamSetC),
            #[cfg(feature = "gost")]
            GOST_34_10_2012_512_A => Ok(Algorithm::Gost3410_2012_512ParamSetA),
            #[cfg(feature = "gost")]
            GOST_34_10_2012_512_B => Ok(Algorithm::Gost3410_2012_512ParamSetB),
            #[cfg(feature = "bls")]
            BLS_NORMAL => Ok(Algorithm::BlsNormal),
            #[cfg(feature = "bls")]
            BLS_SMALL => Ok(Algorithm::BlsSmall),
            #[cfg(feature = "sm")]
            SM2 => Ok(Algorithm::Sm2),
            _ => Err(NoSuchAlgorithm),
        }
    }
}

impl TryFrom<u8> for Algorithm {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Algorithm::Ed25519),
            1 => Ok(Algorithm::Secp256k1),
            4 => Ok(Algorithm::MlDsa),
            #[cfg(feature = "gost")]
            5 => Ok(Algorithm::Gost3410_2012_256ParamSetA),
            #[cfg(feature = "gost")]
            6 => Ok(Algorithm::Gost3410_2012_256ParamSetB),
            #[cfg(feature = "gost")]
            7 => Ok(Algorithm::Gost3410_2012_256ParamSetC),
            #[cfg(feature = "gost")]
            8 => Ok(Algorithm::Gost3410_2012_512ParamSetA),
            #[cfg(feature = "gost")]
            9 => Ok(Algorithm::Gost3410_2012_512ParamSetB),
            #[cfg(feature = "bls")]
            2 => Ok(Algorithm::BlsNormal),
            #[cfg(feature = "bls")]
            3 => Ok(Algorithm::BlsSmall),
            #[cfg(feature = "sm")]
            10 => Ok(Algorithm::Sm2),
            _ => Err(()),
        }
    }
}

#[cfg(not(feature = "ffi_import"))]
impl IntoSchema for Algorithm {
    fn type_name() -> String {
        "Algorithm".into()
    }

    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }

        let mut variants = Vec::new();
        let mut push_variant = |tag: &str, discriminant: u8| {
            variants.push(iroha_schema::EnumVariant {
                tag: tag.to_owned(),
                discriminant,
                ty: None,
            });
        };

        push_variant("Ed25519", Algorithm::Ed25519 as u8);
        push_variant("Secp256k1", Algorithm::Secp256k1 as u8);
        #[cfg(feature = "bls")]
        {
            push_variant("BlsNormal", Algorithm::BlsNormal as u8);
            push_variant("BlsSmall", Algorithm::BlsSmall as u8);
        }
        push_variant("MlDsa", Algorithm::MlDsa as u8);
        #[cfg(feature = "gost")]
        {
            push_variant(
                "Gost3410_2012_256ParamSetA",
                Algorithm::Gost3410_2012_256ParamSetA as u8,
            );
            push_variant(
                "Gost3410_2012_256ParamSetB",
                Algorithm::Gost3410_2012_256ParamSetB as u8,
            );
            push_variant(
                "Gost3410_2012_256ParamSetC",
                Algorithm::Gost3410_2012_256ParamSetC as u8,
            );
            push_variant(
                "Gost3410_2012_512ParamSetA",
                Algorithm::Gost3410_2012_512ParamSetA as u8,
            );
            push_variant(
                "Gost3410_2012_512ParamSetB",
                Algorithm::Gost3410_2012_512ParamSetB as u8,
            );
        }
        #[cfg(feature = "sm")]
        push_variant("Sm2", Algorithm::Sm2 as u8);

        map.insert::<Self>(iroha_schema::Metadata::Enum(iroha_schema::EnumMeta {
            variants,
        }));
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<'a> norito_core::DecodeFromSlice<'a> for Algorithm {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito_core::Error> {
        let (raw, used) = <u8 as norito_core::DecodeFromSlice>::decode_from_slice(bytes)?;
        let algorithm = Algorithm::try_from(raw)
            .map_err(|()| norito_core::Error::invalid_tag("Algorithm::try_from", raw))?;
        Ok((algorithm, used))
    }
}

#[cfg(not(feature = "ffi_import"))]
impl NoritoJsonSerialize for Algorithm {
    fn json_serialize(&self, out: &mut String) {
        let value = self.as_static_str().to_string();
        NoritoJsonSerialize::json_serialize(&value, out);
    }
}

#[cfg(not(feature = "ffi_import"))]
impl NoritoJsonDeserialize for Algorithm {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value: String = NoritoJsonDeserialize::json_deserialize(parser)?;
        Algorithm::from_str(&value)
            .map_err(|_| json::Error::Message(format!("unknown algorithm '{value}'")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conversions_are_consistent() {
        let mut cases = Vec::new();
        cases.push((Algorithm::Ed25519, ED_25519, 0u8));
        cases.push((Algorithm::Secp256k1, SECP_256_K1, 1u8));

        cases.push((Algorithm::MlDsa, ML_DSA, 4u8));

        #[cfg(feature = "gost")]
        cases.extend_from_slice(&[
            (
                Algorithm::Gost3410_2012_256ParamSetA,
                GOST_34_10_2012_256_A,
                5u8,
            ),
            (
                Algorithm::Gost3410_2012_256ParamSetB,
                GOST_34_10_2012_256_B,
                6u8,
            ),
            (
                Algorithm::Gost3410_2012_256ParamSetC,
                GOST_34_10_2012_256_C,
                7u8,
            ),
            (
                Algorithm::Gost3410_2012_512ParamSetA,
                GOST_34_10_2012_512_A,
                8u8,
            ),
            (
                Algorithm::Gost3410_2012_512ParamSetB,
                GOST_34_10_2012_512_B,
                9u8,
            ),
        ]);

        #[cfg(feature = "bls")]
        cases.extend_from_slice(&[
            (Algorithm::BlsNormal, BLS_NORMAL, 2u8),
            (Algorithm::BlsSmall, BLS_SMALL, 3u8),
        ]);

        for (alg, name, value) in cases {
            assert_eq!(alg.as_static_str(), name);
            assert_eq!(Algorithm::from_str(name).unwrap(), alg);
            assert_eq!(Algorithm::try_from(value).unwrap(), alg);
            assert_eq!(alg as u8, value);
        }
    }
}

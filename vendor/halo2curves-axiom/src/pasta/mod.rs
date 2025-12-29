use crate::arithmetic::mul_512;
use crate::arithmetic::sbb;
use crate::{
    arithmetic::{CurveEndo, EndoParameters},
    endo,
};
use crate::serde::SerdeObject;
use ff::PrimeField;
use ff::WithSmallOrderMulGroup;
pub use pasta_curves::{pallas, vesta, Ep, EpAffine, Eq, EqAffine, Fp, Fq};
use std::convert::TryInto;
use std::io::{self, Read, Write};
use group::{Curve, GroupEncoding};

// Generated using https://github.com/ConsenSys/gnark-crypto/blob/master/ecc/utils.go
// with `pasta_curves::Fp::ZETA`
// See https://github.com/demining/Endomorphism-Secp256k1/blob/main/README.md
// to have more details about the endomorphism.
const ENDO_PARAMS_EQ: EndoParameters = EndoParameters {
    // round(b2/n)
    gamma1: [0x32c49e4c00000003, 0x279a745902a2654e, 0x1, 0x0],
    // round(-b1/n)
    gamma2: [0x31f0256800000002, 0x4f34e8b2066389a4, 0x2, 0x0],
    b1: [0x8cb1279300000001, 0x49e69d1640a89953, 0x0, 0x0],
    b2: [0x0c7c095a00000001, 0x93cd3a2c8198e269, 0x0, 0x0],
};

// Generated using https://github.com/ConsenSys/gnark-crypto/blob/master/ecc/utils.go
// with `pasta_curves::Fq::ZETA`
// See https://github.com/demining/Endomorphism-Secp256k1/blob/main/README.md
// to have more details about the endomorphism.
const ENDO_PARAMS_EP: EndoParameters = EndoParameters {
    // round(b2/n)
    gamma1: [0x32c49e4bffffffff, 0x279a745902a2654e, 0x1, 0x0],
    // round(-b1/n)
    gamma2: [0x31f0256800000002, 0x4f34e8b2066389a4, 0x2, 0x0],
    b1: [0x8cb1279300000000, 0x49e69d1640a89953, 0x0, 0x0],
    b2: [0x0c7c095a00000001, 0x93cd3a2c8198e269, 0x0, 0x0],
};

endo!(Eq, Fp, ENDO_PARAMS_EQ);
endo!(Ep, Fq, ENDO_PARAMS_EP);

fn read_32_bytes<R: Read>(reader: &mut R) -> io::Result<[u8; 32]> {
    let mut buf = [0u8; 32];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

impl SerdeObject for Fp {
    fn from_raw_bytes_unchecked(bytes: &[u8]) -> Self {
        assert!(
            bytes.len() == 32,
            "Fp raw encoding must be 32 bytes, got {}",
            bytes.len()
        );
        let mut repr = [0u8; 32];
        repr.copy_from_slice(bytes);
        Option::<Self>::from(Self::from_repr(repr))
            .expect("invalid canonical encoding for Fp")
    }

    fn from_raw_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 32 {
            return None;
        }
        let mut repr = [0u8; 32];
        repr.copy_from_slice(bytes);
        Option::<Self>::from(Self::from_repr(repr))
    }

    fn to_raw_bytes(&self) -> Vec<u8> {
        self.to_repr().as_ref().to_vec()
    }

    fn read_raw_unchecked<R: Read>(reader: &mut R) -> Self {
        let repr = read_32_bytes(reader).expect("Fp read failed");
        Self::from_raw_bytes_unchecked(&repr)
    }

    fn read_raw<R: Read>(reader: &mut R) -> io::Result<Self> {
        let repr = read_32_bytes(reader)?;
        Self::from_raw_bytes(&repr).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid canonical encoding for Fp")
        })
    }

    fn write_raw<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let repr = self.to_repr();
        writer.write_all(repr.as_ref())
    }
}

impl SerdeObject for Fq {
    fn from_raw_bytes_unchecked(bytes: &[u8]) -> Self {
        assert!(
            bytes.len() == 32,
            "Fq raw encoding must be 32 bytes, got {}",
            bytes.len()
        );
        let mut repr = [0u8; 32];
        repr.copy_from_slice(bytes);
        Option::<Self>::from(Self::from_repr(repr))
            .expect("invalid canonical encoding for Fq")
    }

    fn from_raw_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 32 {
            return None;
        }
        let mut repr = [0u8; 32];
        repr.copy_from_slice(bytes);
        Option::<Self>::from(Self::from_repr(repr))
    }

    fn to_raw_bytes(&self) -> Vec<u8> {
        self.to_repr().as_ref().to_vec()
    }

    fn read_raw_unchecked<R: Read>(reader: &mut R) -> Self {
        let repr = read_32_bytes(reader).expect("Fq read failed");
        Self::from_raw_bytes_unchecked(&repr)
    }

    fn read_raw<R: Read>(reader: &mut R) -> io::Result<Self> {
        let repr = read_32_bytes(reader)?;
        Self::from_raw_bytes(&repr).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid canonical encoding for Fq")
        })
    }

    fn write_raw<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let repr = self.to_repr();
        writer.write_all(repr.as_ref())
    }
}

impl SerdeObject for EqAffine {
    fn from_raw_bytes_unchecked(bytes: &[u8]) -> Self {
        assert!(
            bytes.len() == 32,
            "EqAffine raw encoding must be 32 bytes, got {}",
            bytes.len()
        );
        let mut repr = [0u8; 32];
        repr.copy_from_slice(bytes);
        Option::<Self>::from(Self::from_bytes(&repr)).expect("invalid encoding for EqAffine")
    }

    fn from_raw_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 32 {
            return None;
        }
        let mut repr = [0u8; 32];
        repr.copy_from_slice(bytes);
        Option::<Self>::from(Self::from_bytes(&repr))
    }

    fn to_raw_bytes(&self) -> Vec<u8> {
        self.to_bytes().as_ref().to_vec()
    }

    fn read_raw_unchecked<R: Read>(reader: &mut R) -> Self {
        let repr = read_32_bytes(reader).expect("EqAffine read failed");
        Self::from_raw_bytes_unchecked(&repr)
    }

    fn read_raw<R: Read>(reader: &mut R) -> io::Result<Self> {
        let repr = read_32_bytes(reader)?;
        Self::from_raw_bytes(&repr)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid EqAffine encoding"))
    }

    fn write_raw<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let bytes = self.to_bytes();
        writer.write_all(bytes.as_ref())
    }
}

impl SerdeObject for Eq {
    fn from_raw_bytes_unchecked(bytes: &[u8]) -> Self {
        Self::from(EqAffine::from_raw_bytes_unchecked(bytes))
    }

    fn from_raw_bytes(bytes: &[u8]) -> Option<Self> {
        EqAffine::from_raw_bytes(bytes).map(Self::from)
    }

    fn to_raw_bytes(&self) -> Vec<u8> {
        self.to_affine().to_bytes().as_ref().to_vec()
    }

    fn read_raw_unchecked<R: Read>(reader: &mut R) -> Self {
        Self::from(EqAffine::read_raw_unchecked(reader))
    }

    fn read_raw<R: Read>(reader: &mut R) -> io::Result<Self> {
        EqAffine::read_raw(reader).map(Self::from)
    }

    fn write_raw<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let bytes = self.to_affine().to_bytes();
        writer.write_all(bytes.as_ref())
    }
}

impl SerdeObject for EpAffine {
    fn from_raw_bytes_unchecked(bytes: &[u8]) -> Self {
        assert!(
            bytes.len() == 32,
            "EpAffine raw encoding must be 32 bytes, got {}",
            bytes.len()
        );
        let mut repr = [0u8; 32];
        repr.copy_from_slice(bytes);
        Option::<Self>::from(Self::from_bytes(&repr)).expect("invalid encoding for EpAffine")
    }

    fn from_raw_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 32 {
            return None;
        }
        let mut repr = [0u8; 32];
        repr.copy_from_slice(bytes);
        Option::<Self>::from(Self::from_bytes(&repr))
    }

    fn to_raw_bytes(&self) -> Vec<u8> {
        self.to_bytes().as_ref().to_vec()
    }

    fn read_raw_unchecked<R: Read>(reader: &mut R) -> Self {
        let repr = read_32_bytes(reader).expect("EpAffine read failed");
        Self::from_raw_bytes_unchecked(&repr)
    }

    fn read_raw<R: Read>(reader: &mut R) -> io::Result<Self> {
        let repr = read_32_bytes(reader)?;
        Self::from_raw_bytes(&repr)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid EpAffine encoding"))
    }

    fn write_raw<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let bytes = self.to_bytes();
        writer.write_all(bytes.as_ref())
    }
}

impl SerdeObject for Ep {
    fn from_raw_bytes_unchecked(bytes: &[u8]) -> Self {
        Self::from(EpAffine::from_raw_bytes_unchecked(bytes))
    }

    fn from_raw_bytes(bytes: &[u8]) -> Option<Self> {
        EpAffine::from_raw_bytes(bytes).map(Self::from)
    }

    fn to_raw_bytes(&self) -> Vec<u8> {
        self.to_affine().to_bytes().as_ref().to_vec()
    }

    fn read_raw_unchecked<R: Read>(reader: &mut R) -> Self {
        Self::from(EpAffine::read_raw_unchecked(reader))
    }

    fn read_raw<R: Read>(reader: &mut R) -> io::Result<Self> {
        EpAffine::read_raw(reader).map(Self::from)
    }

    fn write_raw<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let bytes = self.to_affine().to_bytes();
        writer.write_all(bytes.as_ref())
    }
}

#[test]
fn test_endo() {
    use ff::Field;
    use rand_core::OsRng;

    for _ in 0..100000 {
        let k = Fp::random(OsRng);
        let (k1, k1_neg, k2, k2_neg) = Eq::decompose_scalar(&k);
        if k1_neg & k2_neg {
            assert_eq!(k, -Fp::from_u128(k1) + Fp::ZETA * Fp::from_u128(k2))
        } else if k1_neg {
            assert_eq!(k, -Fp::from_u128(k1) - Fp::ZETA * Fp::from_u128(k2))
        } else if k2_neg {
            assert_eq!(k, Fp::from_u128(k1) + Fp::ZETA * Fp::from_u128(k2))
        } else {
            assert_eq!(k, Fp::from_u128(k1) - Fp::ZETA * Fp::from_u128(k2))
        }
    }

    for _ in 0..100000 {
        let k = Fp::random(OsRng);
        let (k1, k1_neg, k2, k2_neg) = Eq::decompose_scalar(&k);
        if k1_neg & k2_neg {
            assert_eq!(k, -Fp::from_u128(k1) + Fp::ZETA * Fp::from_u128(k2))
        } else if k1_neg {
            assert_eq!(k, -Fp::from_u128(k1) - Fp::ZETA * Fp::from_u128(k2))
        } else if k2_neg {
            assert_eq!(k, Fp::from_u128(k1) + Fp::ZETA * Fp::from_u128(k2))
        } else {
            assert_eq!(k, Fp::from_u128(k1) - Fp::ZETA * Fp::from_u128(k2))
        }
    }
}

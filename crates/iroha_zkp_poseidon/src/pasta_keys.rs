//! Canonical public-point admission shared by model and paired-Pasta signing consumers.

use halo2curves::{
    CurveAffine,
    group::GroupEncoding,
    pasta::{PallasAffine, VestaAffine},
};

/// Validate the Eq/Pallas and Ep/Vesta public keys in protocol order.
///
/// Decoding uses the prime-order Pasta group implementation. Identity, malformed and
/// noncanonical encodings are refused; no public-key-derived signing fallback exists.
///
/// # Errors
/// Returns the parity whose compressed point is invalid.
pub fn validate_paired_public_keys(eq: &[u8; 32], ep: &[u8; 32]) -> Result<(), &'static str> {
    if !canonical_nonidentity::<PallasAffine>(eq) {
        return Err("Eq/Fp helper key is not a canonical non-identity Pallas point");
    }
    if !canonical_nonidentity::<VestaAffine>(ep) {
        return Err("Ep/Fq helper key is not a canonical non-identity Vesta point");
    }
    Ok(())
}

fn canonical_nonidentity<C: CurveAffine>(bytes: &[u8; 32]) -> bool {
    let mut repr = <C as GroupEncoding>::Repr::default();
    if repr.as_ref().len() != bytes.len() {
        return false;
    }
    repr.as_mut().copy_from_slice(bytes);
    Option::<C>::from(C::from_bytes(&repr))
        .is_some_and(|point| !bool::from(point.is_identity()) && point.to_bytes().as_ref() == bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2curves::{
        ff::{Field, PrimeField},
        group::{Curve, Group},
        pasta::{Fp, Fq, Pallas, Vesta},
    };

    fn encoded<C: CurveAffine>(point: C) -> [u8; 32] {
        point.to_bytes().as_ref().try_into().unwrap()
    }

    #[test]
    fn actual_distinct_public_points_validate_in_both_parities() {
        for scalar in 1_u64..=31 {
            let eq = encoded((Pallas::generator() * Fq::from(scalar)).to_affine());
            let ep = encoded((Vesta::generator() * Fp::from(scalar + 32)).to_affine());
            validate_paired_public_keys(&eq, &ep).unwrap();
        }
    }

    #[test]
    fn identities_invalid_points_and_noncanonical_coordinates_are_rejected() {
        let eq = encoded(Pallas::generator().to_affine());
        let ep = encoded(Vesta::generator().to_affine());
        let identity_eq = encoded(Pallas::identity().to_affine());
        let identity_ep = encoded(Vesta::identity().to_affine());
        for bad in [identity_eq, [0xff; 32]] {
            assert!(validate_paired_public_keys(&bad, &ep).is_err());
        }
        for bad in [identity_ep, [0xff; 32]] {
            assert!(validate_paired_public_keys(&eq, &bad).is_err());
        }
        // p itself is not a canonical field coordinate, even though reduction would yield 0.
        let mut p = (-Fp::ONE).to_repr();
        let mut q = (-Fq::ONE).to_repr();
        for bytes in [&mut p, &mut q] {
            for byte in bytes.as_mut() {
                let (value, carry) = byte.overflowing_add(1);
                *byte = value;
                if !carry {
                    break;
                }
            }
        }
        assert!(validate_paired_public_keys(p.as_ref().try_into().unwrap(), &ep).is_err());
        assert!(validate_paired_public_keys(&eq, q.as_ref().try_into().unwrap()).is_err());
    }
}

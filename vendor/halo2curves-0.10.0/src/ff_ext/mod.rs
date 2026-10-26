pub mod cubic;
pub mod inverse;
pub mod jacobi;
pub mod quadratic;
use subtle::{Choice, ConstantTimeEq};

pub trait Legendre {
    fn legendre(&self) -> i64;

    #[inline(always)]
    fn ct_quadratic_non_residue(&self) -> Choice {
        self.legendre().ct_eq(&-1)
    }

    #[inline(always)]
    fn ct_quadratic_residue(&self) -> Choice {
        // The Legendre symbol returns 0 for 0
        // and 1 for quadratic residues,
        // we consider 0 a square hence quadratic residue.
        self.legendre().ct_ne(&-1)
    }
}

#[macro_export]
macro_rules! extend_field_legendre {
    ($field:ident ) => {
        impl $crate::ff_ext::Legendre for $field {
            #[inline(always)]
            fn legendre(&self) -> i64 {
                self.jacobi()
            }
        }
    };
}

pub trait ExtField: ff::Field {
    const NON_RESIDUE: Self;
    #[must_use]
    fn mul_by_nonresidue(&self) -> Self {
        Self::NON_RESIDUE * self
    }
    fn frobenius_map(&mut self, power: usize);
}

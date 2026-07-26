use std::ops::Deref;

use crate::QuantumCell;

use super::*;
/// SafeType for bool (1 bit).
///
/// This is a separate struct from `CompactSafeType` with the same behavior. Because
/// we know only one [`AssignedValue`] is needed to hold the boolean value, we avoid
/// using `CompactSafeType` to avoid the additional heap allocation from a length 1 vector.
#[derive(Clone, Copy, Debug)]
pub struct SafeBool<F: ScalarField>(pub(super) AssignedValue<F>);

/// SafeType for byte (8 bits).
///
/// This is a separate struct from `CompactSafeType` with the same behavior. Because
/// we know only one [`AssignedValue`] is needed to hold the boolean value, we avoid
/// using `CompactSafeType` to avoid the additional heap allocation from a length 1 vector.
#[derive(Clone, Copy, Debug)]
pub struct SafeByte<F: ScalarField>(pub(super) AssignedValue<F>);

macro_rules! safe_primitive_impls {
    ($SafePrimitive:ty) => {
        impl<F: ScalarField> AsRef<AssignedValue<F>> for $SafePrimitive {
            fn as_ref(&self) -> &AssignedValue<F> {
                &self.0
            }
        }

        impl<F: ScalarField> Borrow<AssignedValue<F>> for $SafePrimitive {
            fn borrow(&self) -> &AssignedValue<F> {
                &self.0
            }
        }

        impl<F: ScalarField> Deref for $SafePrimitive {
            type Target = AssignedValue<F>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<F: ScalarField> From<$SafePrimitive> for AssignedValue<F> {
            fn from(safe_primitive: $SafePrimitive) -> Self {
                safe_primitive.0
            }
        }

        impl<F: ScalarField> From<$SafePrimitive> for QuantumCell<F> {
            fn from(safe_primitive: $SafePrimitive) -> Self {
                QuantumCell::Existing(safe_primitive.0)
            }
        }
    };
}

safe_primitive_impls!(SafeBool<F>);
safe_primitive_impls!(SafeByte<F>);

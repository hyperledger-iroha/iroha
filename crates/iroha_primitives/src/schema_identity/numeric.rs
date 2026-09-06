//! Numeric owned and borrowed helper identities preserve their root projections.
use super::*;
use crate::schema_identity::{record_encode, record_nominal};

pub(crate) fn records() -> Vec<json::Value> {
    let mantissa = BigInt::from_i128(-129);
    vec![
        record_encode(
            &scale_::BigIntView(&mantissa),
            std::any::type_name::<BigInt>(),
        ),
        record_nominal(scale_::NumericScaleHelper {
            mantissa: mantissa.clone(),
            scale: 2,
        }),
        record_encode(
            &scale_::NumericScaleHelperView {
                mantissa: scale_::BigIntView(&mantissa),
                scale: 2,
            },
            std::any::type_name::<scale_::NumericScaleHelperView<'_>>(),
        ),
    ]
}

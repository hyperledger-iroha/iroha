//! Canonical finite error types shared by Kotodama and every IVM host.
use iroha_data_model::smart_contract::manifest::{
    ContractErrorTypeDescriptor, ContractErrorVariantDescriptor,
};

/// Construct the canonical nominal error type for bounded-list mutations.
#[must_use]
pub fn list_error_type() -> ContractErrorTypeDescriptor {
    descriptor(
        "kotodama::ListError",
        &[("IndexOutOfBounds", 1), ("CapacityExceeded", 2)],
    )
}

/// Construct the canonical nominal error type for recoverable numeric-domain failures.
#[must_use]
pub fn numeric_error_type() -> ContractErrorTypeDescriptor {
    descriptor(
        "kotodama::NumericError",
        &[
            ("MantissaOverflow", 1),
            ("ScaleOverflow", 2),
            ("DivisionByZero", 3),
            ("RepeatingDecimal", 4),
            ("ExactDivisionScaleOverflow", 5),
            ("InvalidScale", 6),
            ("InexactConversion", 7),
            ("NegativeQuantity", 8),
            ("QuantityUnderflow", 9),
        ],
    )
}

fn descriptor(identity: &str, variants: &[(&str, u32)]) -> ContractErrorTypeDescriptor {
    ContractErrorTypeDescriptor {
        identity: identity.to_owned(),
        variants: variants
            .iter()
            .map(|(name, code)| ContractErrorVariantDescriptor {
                name: (*name).to_owned(),
                code: *code,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builtin_error_types_are_canonical_and_nominal() {
        let list = list_error_type();
        let numeric = numeric_error_type();
        assert!(list.validate() && numeric.validate());
        assert_ne!(list.schema_hash(), numeric.schema_hash());
        assert_eq!(
            list.variant(1).expect("list variant").name,
            "IndexOutOfBounds"
        );
        assert!(numeric.variant(10).is_none());
    }
}

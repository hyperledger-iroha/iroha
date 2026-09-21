//! Shared policy pricing regressions covering exact decimals, tier order and rejected terms.
use super::*;
use crate::sns::fixtures::default_policy;

fn selector(policy: &SuffixPolicyV1, label: &str) -> NameSelectorV1 {
    NameSelectorV1::new(policy.suffix_id, label).unwrap()
}

#[test]
fn shared_quote_preserves_ordered_tier_selection_and_exact_decimal_rent() {
    let mut policy = default_policy();
    policy.pricing[0].base_price.amount = "0.125".parse().unwrap();
    let mut later = policy.pricing[0].clone();
    later.tier_id = 7;
    later.base_price.amount = "9.5".parse().unwrap();
    policy.pricing.push(later);
    let name = selector(&policy, "developer");
    let quote = quote_lease_price(&policy, &name, 3, None).unwrap();
    assert_eq!(quote.pricing_class, 0);
    assert_eq!(quote.amount, "0.375".parse().unwrap());
    assert_eq!(
        quote.payment_asset,
        payment_asset_definition_id(&policy).unwrap()
    );
    assert_eq!(
        quote_lease_price(&policy, &name, 2, Some(7))
            .unwrap()
            .amount,
        "19".parse().unwrap()
    );
    assert_eq!(tier_by_pricing_class(&policy, &name, 7).unwrap().tier_id, 7);
}

#[test]
fn shared_quote_rejects_inactive_namespace_and_intersected_term_bounds() {
    let mut policy = default_policy();
    let name = selector(&policy, "developer");
    for state in [SuffixStatus::Paused, SuffixStatus::Revoked] {
        policy.status = state;
        assert!(matches!(
            enforce_policy_active(&policy),
            Err(PricingError::Conflict(_))
        ));
        assert!(quote_lease_price(&policy, &name, 1, None).is_err());
    }
    policy.status = SuffixStatus::Active;
    policy.pricing[0].min_duration_years = 2;
    for years in [0, 1, 6] {
        assert!(matches!(
            quote_lease_price(&policy, &name, years, None),
            Err(PricingError::BadRequest(_))
        ));
    }
    assert!(quote_lease_price(&policy, &name, 2, None).is_ok());
    policy.pricing[0].min_duration_years = 6;
    assert!(matches!(
        validate_term_bounds(&policy, &policy.pricing[0], 6),
        Err(PricingError::Conflict(_))
    ));
    let wrong = NameSelectorV1::new(policy.suffix_id + 1, "developer").unwrap();
    assert!(
        quote_lease_price(&policy, &wrong, 1, None)
            .unwrap_err()
            .to_string()
            .contains("suffix ids differ")
    );
}

#[test]
fn shared_quote_rejects_missing_or_malformed_pricing_without_fallback() {
    let mut policy = default_policy();
    let name = selector(&policy, "developer");
    assert!(pick_pricing_tier(&policy, &name, Some(9)).is_err());
    policy.pricing[0].label_regex = "^reserved$".into();
    assert!(pick_pricing_tier(&policy, &name, None).is_err());
    assert!(pick_pricing_tier(&policy, &name, Some(0)).is_err());
    assert!(
        tier_by_pricing_class(&policy, &name, 0)
            .unwrap_err()
            .to_string()
            .contains("no longer satisfies")
    );
    policy.pricing[0].label_regex = "[".into();
    assert!(matches!(
        pick_pricing_tier(&policy, &name, None),
        Err(PricingError::Conflict(_))
    ));
}

#[test]
fn shared_quote_accepts_canonical_asset_forms_and_rejects_invalid_currency() {
    let mut policy = default_policy();
    let name = selector(&policy, "developer");
    let expected = payment_asset_definition_id(&policy).unwrap();
    policy.payment_asset_id = expected.to_string();
    assert_eq!(payment_asset_definition_id(&policy).unwrap(), expected);
    assert_eq!(
        quote_lease_price(&policy, &name, 1, None)
            .unwrap()
            .payment_asset,
        expected
    );
    policy.payment_asset_id = "invalid fee asset".into();
    assert!(matches!(
        payment_asset_definition_id(&policy),
        Err(PricingError::Conflict(_))
    ));
}

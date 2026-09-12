//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::SubscriptionPlan>(
        "iroha_data_model::subscription::SubscriptionPlan",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SubscriptionBilling>(
        "iroha_data_model::subscription::SubscriptionBilling",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SubscriptionMonthlyCalendarCadence>(
        "iroha_data_model::subscription::SubscriptionMonthlyCalendarCadence",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SubscriptionFixedPeriodCadence>(
        "iroha_data_model::subscription::SubscriptionFixedPeriodCadence",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SubscriptionCadence>(
        "iroha_data_model::subscription::SubscriptionCadence",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SubscriptionBillFor>(
        "iroha_data_model::subscription::SubscriptionBillFor",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SubscriptionFixedPricing>(
        "iroha_data_model::subscription::SubscriptionFixedPricing",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SubscriptionUsagePricing>(
        "iroha_data_model::subscription::SubscriptionUsagePricing",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SubscriptionUsageDelta>(
        "iroha_data_model::subscription::SubscriptionUsageDelta",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SubscriptionPricing>(
        "iroha_data_model::subscription::SubscriptionPricing",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SubscriptionState>(
        "iroha_data_model::subscription::SubscriptionState",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SubscriptionStatus>(
        "iroha_data_model::subscription::SubscriptionStatus",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SubscriptionTriggerRef>(
        "iroha_data_model::subscription::SubscriptionTriggerRef",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SubscriptionInvoice>(
        "iroha_data_model::subscription::SubscriptionInvoice",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SubscriptionInvoiceStatus>(
        "iroha_data_model::subscription::SubscriptionInvoiceStatus",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

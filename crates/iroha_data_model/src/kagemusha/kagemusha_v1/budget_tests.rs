//! Exact representability of durable wire-budget components.

use super::*;

#[test]
fn governed_request_cap_fits_the_three_message_exchange() {
    assert_eq!(KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1, 1_024);
    assert_eq!(KAGEMUSHA_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1, 1_371);
    assert!(
        KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1
            + KAGEMUSHA_PAYMENT_MAX_BYTES_V1
            + KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1
            <= KAGEMUSHA_COMPLETE_EXCHANGE_MAX_BYTES_V1
    );
    assert!(
        KAGEMUSHA_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1
            + KAGEMUSHA_PAYMENT_TEXT_MAX_BYTES_V1
            + KAGEMUSHA_ACKNOWLEDGEMENT_TEXT_MAX_BYTES_V1
            <= KAGEMUSHA_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1
    );
}

#[test]
fn durable_budget_components_preserve_the_complete_u32_domain() {
    for value in [0, 1, 65_536, u32::MAX] {
        assert_eq!(
            durable_budget_component_from_usize(usize::try_from(value).unwrap()),
            value
        );
    }
    let expected = u32::try_from(KAGEMUSHA_PAYMENT_MAX_BYTES_V1).unwrap()
        + KAGEMUSHA_INBOX_STAGING_METADATA_MAX_BYTES_V1
        + u32::try_from(KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1).unwrap();
    assert_eq!(KAGEMUSHA_INBOX_STAGE_MIN_BYTES_V1, expected);
}

#[cfg(target_pointer_width = "64")]
#[test]
#[should_panic(expected = "assertion failed: value <= u32::MAX as usize")]
fn durable_budget_components_reject_the_first_unrepresentable_value() {
    durable_budget_component_from_usize(usize::try_from(u64::from(u32::MAX) + 1).unwrap());
}

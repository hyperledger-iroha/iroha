//! Settlement time projections and prepaid-usage error boundaries.
use super::*;

#[test]
fn usage_projection_checks_full_u32_uptime_range_and_rounds_up() {
    let (_, signed) = sample_relay_receipt();
    let (_, mut voucher) = sample_usage_voucher();
    voucher.body.active_ms = u64::MAX;
    let maximum = u64::from(u32::MAX) * 1_000;
    for elapsed in [0, 1, 999, 1_000, 1_001, maximum, maximum + 1] {
        let mut receipt = signed.receipt.clone();
        receipt.ended_at_ms = receipt.started_at_ms + elapsed;
        receipt.uptime_secs = u32::try_from(elapsed.div_ceil(1_000)).unwrap_or(u32::MAX);
        let result = verify_vpn_receipt_usage(&receipt, &voucher.body);
        if elapsed <= maximum {
            assert_eq!(result.unwrap(), elapsed);
        } else {
            assert_eq!(
                result.unwrap_err().to_string(),
                "vpn receipt active time exceeds receipt range"
            );
        }
    }
}

#[test]
fn usage_projection_preserves_interval_ceiling_uptime_and_issuance_error_order() {
    let (_, signed) = sample_relay_receipt();
    let (_, mut voucher) = sample_usage_voucher();
    let mut receipt = signed.receipt;
    receipt.ended_at_ms = receipt.started_at_ms - 1;
    receipt.ingress_bytes = voucher.body.ingress_bytes + 1;
    receipt.uptime_secs = 0;
    voucher.body.issued_at_ms = receipt.started_at_ms + 2;
    assert_eq!(
        verify_vpn_receipt_usage(&receipt, &voucher.body)
            .unwrap_err()
            .to_string(),
        "vpn receipt service interval is inverted"
    );
    receipt.ended_at_ms = receipt.started_at_ms + 1;
    assert_eq!(
        verify_vpn_receipt_usage(&receipt, &voucher.body)
            .unwrap_err()
            .to_string(),
        "vpn receipt usage exceeds the signed prepaid voucher ceilings"
    );
    receipt.ingress_bytes = voucher.body.ingress_bytes;
    assert_eq!(
        verify_vpn_receipt_usage(&receipt, &voucher.body)
            .unwrap_err()
            .to_string(),
        "vpn receipt uptime must equal its observed service interval rounded up"
    );
    receipt.uptime_secs = 1;
    assert_eq!(
        verify_vpn_receipt_usage(&receipt, &voucher.body)
            .unwrap_err()
            .to_string(),
        "vpn receipt ends before the highest prepaid voucher was issued"
    );
    voucher.body.issued_at_ms = receipt.ended_at_ms;
    assert_eq!(
        verify_vpn_receipt_usage(&receipt, &voucher.body).unwrap(),
        1
    );
}

//! Exact settlement formatting, numeric conversions, and formatter failure contracts.

use core::fmt::{self, Display, Write as _};
use settlement_router::{
    BufferPolicy, CollateralKind, EpsilonBps, LiquidityProfile, SwapLineId, VolatilityBucket,
};

struct FailingWriter;

impl fmt::Write for FailingWriter {
    fn write_str(&mut self, _: &str) -> fmt::Result {
        Err(fmt::Error)
    }
}

fn assert_fixed_display(value: &impl Display, expected: &str) {
    assert_eq!(value.to_string(), expected);
    assert_eq!(format!("{value:>100.2}"), expected);
    assert_eq!(format!("{value:🦀^100.2}"), expected);
    assert_eq!(format!("{value:+#0100.2}"), expected);
    assert_eq!(write!(FailingWriter, "{value}"), Err(fmt::Error));
}

#[test]
fn enum_display_preserves_every_spelling_and_ignores_outer_flags() {
    for (value, expected) in [
        (LiquidityProfile::Tier1, "tier1-deep"),
        (LiquidityProfile::Tier2, "tier2-medium"),
        (LiquidityProfile::Tier3, "tier3-thin"),
    ] {
        assert_fixed_display(&value, expected);
    }
    for (value, expected) in [
        (CollateralKind::Cbdc, "cbdc"),
        (CollateralKind::Xor, "xor"),
        (CollateralKind::Stable, "stable"),
    ] {
        assert_fixed_display(&value, expected);
    }
    for (value, expected) in [
        (VolatilityBucket::Stable, "stable"),
        (VolatilityBucket::Elevated, "elevated"),
        (VolatilityBucket::Dislocated, "dislocated"),
    ] {
        assert_fixed_display(&value, expected);
    }
}

#[test]
fn epsilon_display_preserves_units_and_full_unsigned_domain() {
    for (raw, expected) in [
        (0, "0 bps"),
        (1, "1 bps"),
        (25, "25 bps"),
        (10_000, "10000 bps"),
        (u16::MAX, "65535 bps"),
    ] {
        assert_fixed_display(&EpsilonBps::new(raw), expected);
    }
}

#[test]
fn swap_line_id_conversion_and_display_preserve_integer_formatting() {
    for raw in [0, 1, 42, u32::MAX] {
        let id = SwapLineId::from(raw);
        assert_eq!(id.0, raw);
        assert_eq!(format!("{id}"), format!("{raw}"));
        assert_eq!(format!("{id:>40.2}"), format!("{raw:>40.2}"));
        assert_eq!(format!("{id:🦀^40.2}"), format!("{raw:🦀^40.2}"));
        assert_eq!(format!("{id:+#040.2}"), format!("{raw:+#040.2}"));
        assert_eq!(write!(FailingWriter, "{id}"), Err(fmt::Error));
    }
}

#[test]
fn buffer_policy_tuple_conversion_preserves_order_without_validation() {
    for (raw, expected) in [
        (
            (75, 25, 10, 2),
            "alert 75%, throttle 25%, xor-only 10%, halt 2%",
        ),
        ((0, 0, 0, 0), "alert 0%, throttle 0%, xor-only 0%, halt 0%"),
        (
            (0, 255, 1, 254),
            "alert 0%, throttle 255%, xor-only 1%, halt 254%",
        ),
    ] {
        let policy = BufferPolicy::from(raw);
        assert_eq!(
            (policy.alert, policy.throttle, policy.xor_only, policy.halt),
            raw,
        );
        assert_fixed_display(&policy, expected);
    }
    assert!(BufferPolicy::from((0, 255, 1, 254)).validate().is_err());
}

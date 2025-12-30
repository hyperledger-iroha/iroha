//! Deterministic settlement receipts serialised via Norito.

use std::convert::TryFrom;

use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{JsonDeserialize, JsonSerialize},
};
use time::OffsetDateTime;

use crate::{MicroXor, ShadowPrice};

/// Receipt produced once a transaction has been admitted with an associated
/// shadow price.  The structure is designed to be serialised via Norito and
/// committed to nightly reconciliation reports without leaking implementation
/// details about the conversion path.
#[derive(
    Clone, Debug, Eq, PartialEq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct SettlementReceipt {
    /// Identifier emitted by the caller (e.g., transaction hash).
    pub source_id: [u8; 32],
    /// Local gas token amount debited from the user or sponsor.
    pub local_amount_micro: u128,
    /// XOR amount booked immediately.
    pub xor_due: MicroXor,
    /// Expected XOR after applying the configured haircut.
    pub xor_with_haircut: MicroXor,
    /// Timestamp (UTC) when the receipt was generated.
    pub timestamp: crate::TimestampMs,
}

impl SettlementReceipt {
    /// Build a new receipt using the provided shadow price and a supplied UTC timestamp in milliseconds.
    #[must_use]
    pub fn new_with_timestamp_ms(
        source_id: [u8; 32],
        local_amount_micro: u128,
        shadow: &ShadowPrice,
        timestamp_ms: u64,
    ) -> Self {
        let nanos = i128::from(timestamp_ms) * 1_000_000;
        let timestamp = OffsetDateTime::from_unix_timestamp_nanos(nanos)
            .expect("timestamp within supported range");
        Self {
            source_id,
            local_amount_micro,
            xor_due: shadow.xor_due,
            xor_with_haircut: shadow.xor_with_haircut,
            timestamp: crate::TimestampMs::from(timestamp),
        }
    }

    /// Build a new receipt using the provided shadow price and the current UTC
    /// timestamp (rounded to milliseconds to keep serialised output stable).
    #[must_use]
    pub fn new(source_id: [u8; 32], local_amount_micro: u128, shadow: &ShadowPrice) -> Self {
        let now = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("nanosecond must be in range");
        let timestamp_ms_i128 = now.unix_timestamp_nanos() / 1_000_000;
        let clamped_timestamp_ms = timestamp_ms_i128.clamp(0, i128::from(u64::MAX));
        let timestamp_ms =
            u64::try_from(clamped_timestamp_ms).expect("timestamp is clamped to the u64 range");
        Self::new_with_timestamp_ms(source_id, local_amount_micro, shadow, timestamp_ms)
    }
}

#[cfg(test)]
mod tests {
    use norito::{decode_from_bytes, json};
    use rust_decimal::Decimal;
    use time::Duration;

    use crate::{
        config::{EpsilonBps, SettlementConfig},
        haircut::{HaircutTier, LiquidityProfile},
        price::ShadowPriceCalculator,
        receipt::SettlementReceipt,
        volatility::VolatilityBucket,
    };

    #[test]
    fn receipt_rounds_timestamp() {
        let calculator = ShadowPriceCalculator::new(SettlementConfig {
            twap_window: crate::DurationSeconds::new(Duration::seconds(60)),
            epsilon: EpsilonBps::new(25),
            buffer_horizon_hours: 72,
        });
        let shadow = calculator.compute(
            Decimal::new(1_000_000, 0),
            Decimal::new(100, 0),
            HaircutTier::new(LiquidityProfile::Tier1),
            VolatilityBucket::Stable,
        );
        let receipt = SettlementReceipt::new([0xAA; 32], 1_000_000, &shadow);

        assert_eq!(receipt.timestamp.as_offset_datetime().millisecond(), 0);
        assert_eq!(receipt.source_id, [0xAA; 32]);
        assert_eq!(receipt.local_amount_micro, 1_000_000);
    }

    #[test]
    fn receipt_norito_roundtrip() {
        let calculator = ShadowPriceCalculator::new(SettlementConfig {
            twap_window: crate::DurationSeconds::new(Duration::seconds(30)),
            epsilon: EpsilonBps::new(15),
            buffer_horizon_hours: 24,
        });
        let shadow = calculator.compute(
            Decimal::new(500_000, 0),
            Decimal::new(50, 0),
            HaircutTier::new(LiquidityProfile::Tier1),
            VolatilityBucket::Stable,
        );
        let receipt =
            SettlementReceipt::new_with_timestamp_ms([0x11; 32], 500_000, &shadow, 1_687_123_456);

        let bytes = norito::to_bytes(&receipt).expect("encode");
        let decoded: SettlementReceipt = decode_from_bytes(&bytes).expect("decode");
        assert_eq!(decoded, receipt);

        let json_text = json::to_json(&receipt).expect("json encode");
        let parsed: SettlementReceipt = json::from_str(&json_text).expect("json decode");
        assert_eq!(parsed, receipt);
    }
}

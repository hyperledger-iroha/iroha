//! Deterministic settlement receipts serialised via Norito.

use std::convert::TryFrom;

use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{JsonDeserialize, JsonSerialize},
};
use time::OffsetDateTime;

use crate::{Quantity, ShadowPrice};

/// Receipt construction failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SettlementReceiptError {
    /// Millisecond timestamp is outside the canonical timestamp domain.
    #[error("settlement receipt timestamp is out of range")]
    TimestampOutOfRange,
}

/// Receipt produced once a transaction has been admitted with an associated
/// shadow price.
#[derive(
    Clone, Debug, Eq, PartialEq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct SettlementReceipt {
    /// Identifier emitted by the caller (for example, a transaction hash).
    pub source_id: [u8; 32],
    /// Exact local gas-token amount debited from the user or sponsor.
    pub local_amount: Quantity,
    /// Exact XOR amount booked immediately.
    pub xor_due: crate::XorQuantity,
    /// Exact XOR expected after applying the configured haircut.
    pub xor_with_haircut: crate::XorQuantity,
    /// Timestamp (UTC) when the receipt was generated.
    pub timestamp: crate::TimestampMs,
}

impl SettlementReceipt {
    /// Build a receipt using a supplied UTC timestamp in milliseconds.
    ///
    /// # Errors
    /// Rejects timestamps outside the canonical `time` domain.
    pub fn new_with_timestamp_ms(
        source_id: [u8; 32],
        local_amount: Quantity,
        shadow: &ShadowPrice,
        timestamp_ms: u64,
    ) -> Result<Self, SettlementReceiptError> {
        let timestamp = crate::TimestampMs::from_unix_millis(timestamp_ms)
            .map_err(|_| SettlementReceiptError::TimestampOutOfRange)?;
        Ok(Self {
            source_id,
            local_amount,
            xor_due: shadow.xor_due.clone(),
            xor_with_haircut: shadow.xor_with_haircut.clone(),
            timestamp,
        })
    }

    /// Build a receipt using the current UTC timestamp rounded to milliseconds.
    #[must_use]
    pub fn new(source_id: [u8; 32], local_amount: Quantity, shadow: &ShadowPrice) -> Self {
        let now = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("nanosecond must be in range");
        let timestamp_ms_i128 = now.unix_timestamp_nanos() / 1_000_000;
        let clamped_timestamp_ms = timestamp_ms_i128.clamp(0, i128::from(u64::MAX));
        let timestamp_ms =
            u64::try_from(clamped_timestamp_ms).expect("timestamp is clamped to the u64 range");
        Self::new_with_timestamp_ms(source_id, local_amount, shadow, timestamp_ms)
            .expect("current UTC timestamp is representable")
    }
}

#[cfg(test)]
mod tests {
    use norito::{decode_from_bytes, json};
    use time::Duration;

    use crate::{
        Numeric, Quantity,
        config::{EpsilonBps, SettlementConfig},
        haircut::{HaircutTier, LiquidityProfile},
        price::ShadowPriceCalculator,
        receipt::{SettlementReceipt, SettlementReceiptError},
        volatility::VolatilityBucket,
    };

    fn shadow(local: &Quantity) -> crate::ShadowPrice {
        ShadowPriceCalculator::new(SettlementConfig {
            twap_window: crate::DurationSeconds::new(Duration::seconds(60)),
            epsilon: EpsilonBps::new(25),
            buffer_horizon_hours: 72,
        })
        .compute(
            local,
            &Numeric::from(100_u32),
            HaircutTier::new(LiquidityProfile::Tier1),
            VolatilityBucket::Stable,
        )
        .expect("valid shadow price")
    }

    #[test]
    fn receipt_rounds_timestamp() {
        let local = Quantity::from(1_000_000_u64);
        let receipt = SettlementReceipt::new([0xAA; 32], local.clone(), &shadow(&local));

        assert_eq!(receipt.timestamp.as_offset_datetime().millisecond(), 0);
        assert_eq!(receipt.source_id, [0xAA; 32]);
        assert_eq!(receipt.local_amount, local);
    }

    #[test]
    fn receipt_norito_roundtrip_preserves_wide_fractional_amounts() {
        let local: Quantity = "340282366920938463463374607431768211456.125"
            .parse()
            .expect("wide canonical quantity");
        let receipt = SettlementReceipt::new_with_timestamp_ms(
            [0x11; 32],
            local.clone(),
            &shadow(&local),
            1_687_123_456,
        )
        .expect("timestamp");

        let bytes = norito::to_bytes(&receipt).expect("encode");
        let decoded: SettlementReceipt = decode_from_bytes(&bytes).expect("decode");
        assert_eq!(decoded, receipt);

        let json_text = json::to_json(&receipt).expect("json encode");
        let parsed: SettlementReceipt = json::from_str(&json_text).expect("json decode");
        assert_eq!(parsed, receipt);
        assert!(json_text.contains(&format!("\"{local}\"")));
    }

    #[test]
    fn receipt_rejects_out_of_range_timestamp_without_panicking() {
        let local = Quantity::one();
        assert_eq!(
            SettlementReceipt::new_with_timestamp_ms(
                [0; 32],
                local.clone(),
                &shadow(&local),
                u64::MAX,
            ),
            Err(SettlementReceiptError::TimestampOutOfRange)
        );
    }
}

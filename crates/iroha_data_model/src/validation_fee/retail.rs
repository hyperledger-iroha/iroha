//! Canonical retail monthly accounting and signed assessments.
use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize, account::AccountId, asset::AssetDefinitionId,
};
use iroha_crypto::Hash;
use iroha_model_base::state_path::StatePath;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use time::{Date, Month, OffsetDateTime, Time, UtcOffset};

/// Signed transaction metadata key for the reviewed fee assessment.
pub const RETAIL_FEE_ASSESSMENT_METADATA_KEY: &str = "validation_fee_assessment";
/// Protected account enrollment metadata key.
pub const RETAIL_FEE_ENROLLMENT_METADATA_KEY: &str = "retail_fee_enrollment";
/// Minimum public notice in milliseconds (thirty Honiara calendar days).
pub const RETAIL_FEE_NOTICE_MS: u64 = 30 * 86_400_000;
/// Maximum lifetime of a reviewed payment assessment.
pub const RETAIL_FEE_QUOTE_TTL_MS: u64 = 60_000;
/// Maximum completed months evaluated for one active wallet in one settlement.
pub const RETAIL_FEE_MAX_CATCH_UP_MONTHS: usize = 12;
/// Temporary rejection while deterministic consensus sweeps catch up old months.
pub const RETAIL_FEE_CATCH_UP_REQUIRED: &str = "LEDGER_CATCH_UP_REQUIRED";

/// A lower-inclusive balance tier, denominated entirely in SBD cents.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::validation_fee::MaintenanceTierV1")]
pub struct MaintenanceTierV1 {
    /// Minimum time-weighted average balance in minor units.
    pub minimum_average_balance_minor: u64,
    /// Full-calendar-month maintenance in minor units.
    pub monthly_fee_minor: u64,
}
/// Parliament-enacted retail pricing; every field is required on the wire.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::validation_fee::RetailFeeScheduleV1")]
pub struct RetailFeeScheduleV1 {
    /// Successful outgoing payments included per canonical wallet per month.
    pub included_payments: u32,
    /// Price of each additional qualifying payment in minor units.
    pub overage_minor: u64,
    /// Strictly ordered lower-inclusive balance thresholds.
    pub maintenance_tiers: Vec<MaintenanceTierV1>,
}
impl Default for RetailFeeScheduleV1 {
    fn default() -> Self {
        Self {
            included_payments: 50,
            overage_minor: 10,
            maintenance_tiers: [
                (0, 100),
                (50_000, 200),
                (250_000, 300),
                (1_000_000, 500),
                (5_000_000, 1_000),
            ]
            .into_iter()
            .map(
                |(minimum_average_balance_minor, monthly_fee_minor)| MaintenanceTierV1 {
                    minimum_average_balance_minor,
                    monthly_fee_minor,
                },
            )
            .collect(),
        }
    }
}
impl RetailFeeScheduleV1 {
    /// Validate exact positive rates and unambiguous tier ordering.
    ///
    /// # Errors
    ///
    /// Returns an error for zero payment allowances or rates, missing or excessive
    /// tiers, a nonzero first threshold, or invalid tier ordering.
    pub fn validate(&self) -> Result<(), String> {
        if self.included_payments == 0
            || self.overage_minor == 0
            || self.maintenance_tiers.is_empty()
            || self.maintenance_tiers.len() > 32
            || self.maintenance_tiers[0].minimum_average_balance_minor != 0
        {
            return Err(
                "retail fees require positive overage and a zero-floor maintenance tier".into(),
            );
        }
        for (index, tier) in self.maintenance_tiers.iter().enumerate() {
            if tier.monthly_fee_minor == 0
                || (index > 0
                    && (tier.minimum_average_balance_minor
                        <= self.maintenance_tiers[index - 1].minimum_average_balance_minor
                        || tier.monthly_fee_minor
                            < self.maintenance_tiers[index - 1].monthly_fee_minor))
            {
                return Err("maintenance tiers must have increasing thresholds and positive nondecreasing charges".into());
            }
        }
        Ok(())
    }
    /// Determine the tier without rounding the average across a threshold.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid schedule or a missing applicable maintenance tier.
    pub fn monthly_fee(&self, balance_time_minor_ms: u128, active_ms: u64) -> Result<u64, String> {
        self.validate()?;
        if active_ms == 0 {
            return Ok(0);
        }
        Ok(self
            .maintenance_tiers
            .iter()
            .rev()
            .find(|tier| {
                u128::from(tier.minimum_average_balance_minor) * u128::from(active_ms)
                    <= balance_time_minor_ms
            })
            .ok_or("missing maintenance tier")?
            .monthly_fee_minor)
    }
    /// Charge payment legs that lie beyond the monthly inclusion.
    ///
    /// # Errors
    ///
    /// Returns an error if the payment counter or computed charge overflows.
    pub fn payment_fee(&self, used: u64, count: u64) -> Result<u64, String> {
        let after = used.checked_add(count).ok_or("payment count overflow")?;
        let included = u64::from(self.included_payments);
        after
            .saturating_sub(included)
            .checked_sub(used.saturating_sub(included))
            .and_then(|billable| billable.checked_mul(self.overage_minor))
            .ok_or_else(|| "payment fee overflow".into())
    }
}
/// Immutable maintenance evidence; a waived amount is never a future receivable.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::validation_fee::RetailMaintenanceReceiptV1")]
pub struct RetailMaintenanceReceiptV1 {
    /// Calendar-month boundary identifying the earning period.
    pub billing_month_start_ms: u64,
    /// Logical collection boundary (or account closure).
    pub effective_at_ms: u64,
    /// Governing revision for this earning period.
    pub policy_revision: u64,
    /// Prorated scheduled amount.
    pub scheduled_minor: u64,
    /// Amount actually collected from available funds.
    pub collected_minor: u64,
    /// Permanently waived amount.
    pub waived_minor: u64,
}
/// Consensus-owned monthly state keyed by a canonical ledger account.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::validation_fee::RetailFeeAccountStateV1")]
pub struct RetailFeeAccountStateV1 {
    /// Stable ledger wallet identity.
    pub account_id: AccountId,
    /// Start of the current Honiara calendar month.
    pub billing_month_start_ms: u64,
    /// End of the current Honiara calendar month.
    pub billing_month_end_ms: u64,
    /// Beginning of active time within this period.
    pub active_from_ms: u64,
    /// Last integrated ledger timestamp.
    pub last_accrual_ms: u64,
    /// Exact integral of balance in minor units over milliseconds.
    pub balance_time_minor_ms: u128,
    /// Accumulated active milliseconds, excluding periods when the wallet was closed.
    pub active_time_ms: u64,
    /// Balance after the last observed mutation or logical deduction.
    pub balance_minor: u64,
    /// Successful qualifying payment legs in this month.
    pub payments_used: u64,
    /// Immutable enrollment timestamp, retained across recovery and reopening.
    pub enrolled_at_ms: u64,
    /// End of the active interval; only that completed calendar month can still be billed.
    pub closed_at_ms: Option<u64>,
    /// Most recently settled month for statements.
    pub last_maintenance: Option<RetailMaintenanceReceiptV1>,
}
/// Canonical successful payment leg bound by the customer's reviewed quote.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::validation_fee::RetailFeePaymentLegV1")]
pub struct RetailFeePaymentLegV1 {
    /// Canonical recipient account.
    pub destination_account_id: AccountId,
    /// Positive payment amount in fee-asset minor units.
    pub amount_minor_units: u64,
}
/// Quote intent excludes mutable counters: they come exclusively from the ledger.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::validation_fee::RetailFeeQuoteRequestV1")]
pub struct RetailFeeQuoteRequestV1 {
    /// Canonical source wallet.
    pub account_id: AccountId,
    /// Governed SBD asset definition.
    pub asset_definition_id: AssetDefinitionId,
    /// Ordered payment legs (including same-account legs, which do not consume allowance).
    pub transfers: Vec<RetailFeePaymentLegV1>,
}
/// Reviewed, transaction-bound fee; included payments explicitly carry zero.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::validation_fee::RetailFeeAssessmentV1")]
pub struct RetailFeeAssessmentV1 {
    /// Account whose allowance and funds will be charged.
    pub account_id: AccountId,
    /// Whether protected ledger enrollment grants the retail monthly inclusion.
    pub retail_enrolled: bool,
    /// Current Honiara month boundary.
    pub billing_month_start_ms: u64,
    /// Parliament policy revision.
    pub policy_revision: u64,
    /// Ledger counter observed before the intended payments.
    pub payments_used_before: u64,
    /// Number of chargeable outgoing legs.
    pub qualifying_payments: u64,
    /// Exact assessed amount; zero does not require a treasury transfer.
    pub fee_minor: u64,
    /// Hash of authoritative account/month/counter/policy facts.
    pub state_commitment: [u8; 32],
    /// Hash of the exact ordered payment intent.
    pub intent_hash: [u8; 32],
    /// Exclusive expiration bound, never beyond month rollover.
    pub expires_at_ms: u64,
}
/// Derive a Honiara calendar-month interval, UTC+11 without DST.
///
/// # Errors
///
/// Returns an error if the timestamp or either month boundary is outside the
/// supported calendar or unsigned epoch.
pub fn honiara_month_bounds(timestamp_ms: u64) -> Result<(u64, u64), String> {
    let utc = OffsetDateTime::from_unix_timestamp_nanos(i128::from(timestamp_ms) * 1_000_000)
        .map_err(|_| "ledger timestamp outside calendar domain")?;
    let local = utc.to_offset(UtcOffset::from_hms(11, 0, 0).map_err(|_| "invalid Honiara offset")?);
    let (year, month, _) = local.to_calendar_date();
    let start = Date::from_calendar_date(year, month, 1)
        .map_err(|_| "invalid month")?
        .with_time(Time::MIDNIGHT)
        .assume_offset(local.offset());
    let (next_year, next_month) = if month == Month::December {
        (
            year.checked_add(1).ok_or("calendar overflow")?,
            Month::January,
        )
    } else {
        (year, month.next())
    };
    let end = Date::from_calendar_date(next_year, next_month, 1)
        .map_err(|_| "invalid next month")?
        .with_time(Time::MIDNIGHT)
        .assume_offset(local.offset());
    Ok((
        u64::try_from(start.unix_timestamp_nanos() / 1_000_000)
            .map_err(|_| "month precedes epoch")?,
        u64::try_from(end.unix_timestamp_nanos() / 1_000_000)
            .map_err(|_| "month end outside epoch")?,
    ))
}
/// Require a whole-month activation following at least thirty days' notice.
///
/// # Errors
///
/// Returns an error for insufficient notice, timestamp overflow, or activation
/// outside an exact Honiara month boundary.
pub fn validate_retail_activation(notice_ms: u64, effective_ms: u64) -> Result<(), String> {
    if notice_ms
        .checked_add(RETAIL_FEE_NOTICE_MS)
        .is_none_or(|minimum| effective_ms < minimum)
        || honiara_month_bounds(effective_ms)?.0 != effective_ms
    {
        return Err("retail policy activation requires a Honiara month boundary and at least 30 calendar days notice".into());
    }
    Ok(())
}
impl RetailFeeAccountStateV1 {
    /// Create protected enrollment; callers must authorize enrollment on chain.
    ///
    /// # Errors
    ///
    /// Returns an error if the enrollment timestamp is outside the supported calendar range.
    pub fn enroll(account_id: AccountId, now_ms: u64, balance_minor: u64) -> Result<Self, String> {
        let (start, end) = honiara_month_bounds(now_ms)?;
        Ok(Self {
            account_id,
            billing_month_start_ms: start,
            billing_month_end_ms: end,
            active_from_ms: now_ms,
            last_accrual_ms: now_ms,
            balance_time_minor_ms: 0,
            active_time_ms: 0,
            balance_minor,
            payments_used: 0,
            enrolled_at_ms: now_ms,
            closed_at_ms: None,
            last_maintenance: None,
        })
    }
    /// Integrate balances and settle expired months before a later balance mutation.
    /// The callback returns the policy governing the earning period. Collection is
    /// bounded by that boundary's available funds, and every shortfall expires.
    ///
    /// # Errors
    ///
    /// Returns an error for backward time, excessive catch-up, calendar or arithmetic
    /// overflow, or a policy callback or maintenance assessment failure.
    pub fn settle_until(
        &mut self,
        now_ms: u64,
        available: bool,
        mut schedule_for: impl FnMut(u64) -> Result<(u64, RetailFeeScheduleV1), String>,
    ) -> Result<Vec<RetailMaintenanceReceiptV1>, String> {
        if now_ms < self.last_accrual_ms {
            return Err("ledger clock moved backwards".into());
        }
        // Bound work before changing even the in-memory candidate. Closed wallets
        // produce at most one closing-month receipt and then fast-forward below.
        if self.closed_at_ms.is_none() {
            let mut first_unbounded_boundary = self.billing_month_end_ms;
            for _ in 0..RETAIL_FEE_MAX_CATCH_UP_MONTHS {
                if now_ms < first_unbounded_boundary {
                    break;
                }
                first_unbounded_boundary = honiara_month_bounds(first_unbounded_boundary)?.1;
            }
            if now_ms >= first_unbounded_boundary {
                return Err(RETAIL_FEE_CATCH_UP_REQUIRED.into());
            }
        }
        let mut receipts = Vec::new();
        loop {
            let to = now_ms.min(self.billing_month_end_ms);
            let delta = to
                .checked_sub(self.last_accrual_ms)
                .ok_or("invalid accrual interval")?;
            if self.closed_at_ms.is_none() {
                self.balance_time_minor_ms = self
                    .balance_time_minor_ms
                    .checked_add(u128::from(self.balance_minor) * u128::from(delta))
                    .ok_or("balance-time overflow")?;
                self.active_time_ms = self
                    .active_time_ms
                    .checked_add(delta)
                    .ok_or("active-time overflow")?;
            }
            self.last_accrual_ms = to;
            if to < self.billing_month_end_ms {
                break;
            }
            if self.active_time_ms > 0 {
                let (revision, schedule) = schedule_for(self.active_from_ms)?;
                let receipt = self.assess_maintenance(to, revision, &schedule, available)?;
                self.balance_minor -= receipt.collected_minor;
                self.last_maintenance = Some(receipt);
                receipts.push(receipt);
            }
            let (start, end) = honiara_month_bounds(to)?;
            self.billing_month_start_ms = start;
            self.billing_month_end_ms = end;
            self.active_from_ms = start;
            self.balance_time_minor_ms = 0;
            self.active_time_ms = 0;
            self.payments_used = 0;
            if self.closed_at_ms.is_some() {
                // Fully inactive months create no bills or receipt growth. Preserve the closed
                // state while advancing the calendar so reopening starts with this month's quota.
                let (start, end) = honiara_month_bounds(now_ms)?;
                self.billing_month_start_ms = start;
                self.billing_month_end_ms = end;
                self.active_from_ms = start;
                self.last_accrual_ms = now_ms;
                break;
            }
            if now_ms == to {
                break;
            }
        }
        Ok(receipts)
    }
    /// Compute a prorated charge, rounding upward to one cent.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid fee schedule, missing maintenance tier, or
    /// charge arithmetic overflow.
    pub fn assess_maintenance(
        &self,
        at_ms: u64,
        revision: u64,
        schedule: &RetailFeeScheduleV1,
        available: bool,
    ) -> Result<RetailMaintenanceReceiptV1, String> {
        let active_ms = self.active_time_ms;
        let month_ms = self.billing_month_end_ms - self.billing_month_start_ms;
        let monthly = schedule.monthly_fee(self.balance_time_minor_ms, active_ms)?;
        let numerator = u128::from(monthly) * u128::from(active_ms);
        let scheduled = u64::try_from(numerator.div_ceil(u128::from(month_ms)))
            .map_err(|_| "maintenance overflow")?;
        let collected = if available {
            scheduled.min(self.balance_minor)
        } else {
            0
        };
        Ok(RetailMaintenanceReceiptV1 {
            billing_month_start_ms: self.billing_month_start_ms,
            effective_at_ms: at_ms,
            policy_revision: revision,
            scheduled_minor: scheduled,
            collected_minor: collected,
            waived_minor: scheduled - collected,
        })
    }
    /// Reopen the same canonical wallet without granting another allowance in the same month.
    ///
    /// # Errors
    ///
    /// Returns an error for backward time, an unsettled closing month, or a timestamp
    /// outside the supported calendar range.
    pub fn reopen(&mut self, now_ms: u64, balance_minor: u64) -> Result<(), String> {
        if self.closed_at_ms.is_none() {
            return Ok(());
        }
        if now_ms < self.last_accrual_ms {
            return Err("reopening precedes prior closure".into());
        }
        let (start, end) = honiara_month_bounds(now_ms)?;
        if start != self.billing_month_start_ms {
            if self.active_time_ms != 0 {
                return Err("closing month must settle before reopening".into());
            }
            self.billing_month_start_ms = start;
            self.billing_month_end_ms = end;
            self.active_from_ms = now_ms;
            self.active_time_ms = 0;
            self.balance_time_minor_ms = 0;
            self.payments_used = 0;
        }
        if self.active_time_ms == 0 {
            self.active_from_ms = now_ms;
        }
        self.last_accrual_ms = now_ms;
        self.balance_minor = balance_minor;
        self.closed_at_ms = None;
        Ok(())
    }
    /// Bind a quote to the current allowance and policy, independent of elapsed milliseconds.
    ///
    /// # Errors
    ///
    /// Returns an error if the quote context cannot be canonically encoded.
    pub fn state_commitment(
        &self,
        policy_hash: [u8; 32],
        retail_enrolled: bool,
    ) -> Result<[u8; 32], String> {
        let payload = norito::encode_canonical(&(
            self.account_id.clone(),
            self.billing_month_start_ms,
            self.payments_used,
            policy_hash,
            retail_enrolled,
        ))
        .map_err(|e| e.to_string())?;
        Ok(domain_hash(b"iroha.retail_fee.account_state.v1", &payload))
    }
}
fn domain_hash(domain: &[u8], payload: &[u8]) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(domain.len() + 1 + payload.len());
    bytes.extend_from_slice(domain);
    bytes.push(0);
    bytes.extend_from_slice(payload);
    *Hash::new(bytes).as_ref()
}
impl RetailFeeQuoteRequestV1 {
    /// Hash the canonical fee-asset payment intent.
    ///
    /// # Errors
    ///
    /// Returns an error if the payment intent cannot be canonically encoded.
    pub fn intent_hash(&self) -> Result<[u8; 32], String> {
        Ok(domain_hash(
            b"iroha.retail_fee.payment_intent.v1",
            &norito::encode_canonical(self).map_err(|e| e.to_string())?,
        ))
    }
    /// Count successful positive transfers to another canonical account.
    pub fn qualifying_payments(&self) -> u64 {
        self.transfers
            .iter()
            .filter(|leg| {
                leg.amount_minor_units > 0 && leg.destination_account_id != self.account_id
            })
            .count() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn at(year: i32, month: Month, day: u8) -> u64 {
        u64::try_from(
            Date::from_calendar_date(year, month, day)
                .unwrap()
                .with_time(Time::MIDNIGHT)
                .assume_offset(UtcOffset::from_hms(11, 0, 0).unwrap())
                .unix_timestamp_nanos()
                / 1_000_000,
        )
        .unwrap()
    }
    fn account() -> AccountId {
        let pair = iroha_crypto::KeyPair::random();
        AccountId::new(pair.public_key().clone())
    }
    #[test]
    fn launch_tiers_and_payment_boundaries() {
        let s = RetailFeeScheduleV1::default();
        s.validate().unwrap();
        for (balance, fee) in [
            (0, 100),
            (49_999, 100),
            (50_000, 200),
            (249_999, 200),
            (250_000, 300),
            (1_000_000, 500),
            (5_000_000, 1000),
        ] {
            assert_eq!(
                s.monthly_fee(
                    u128::from(u64::try_from(balance).expect("nonnegative test balance")) * 100,
                    100
                )
                .unwrap(),
                fee
            );
        }
        assert_eq!(s.payment_fee(49, 3).unwrap(), 20);
        assert_eq!(s.payment_fee(0, 50).unwrap(), 0);
        assert_eq!(s.payment_fee(50, 1).unwrap(), 10);
        assert_eq!(s.payment_fee(51, 1).unwrap(), 10);
    }
    #[test]
    fn rejects_ambiguous_and_zero_tariffs() {
        let mut s = RetailFeeScheduleV1::default();
        s.maintenance_tiers[1].minimum_average_balance_minor = 0;
        assert!(s.validate().is_err());
        s = RetailFeeScheduleV1::default();
        s.maintenance_tiers[0].monthly_fee_minor = 0;
        assert!(s.validate().is_err());
    }
    #[test]
    fn honiara_month_and_leap_year() {
        let start = at(2024, Month::February, 1);
        let end = at(2024, Month::March, 1);
        assert_eq!(end - start, 29 * 86_400_000);
        assert_eq!(honiara_month_bounds(end - 1).unwrap(), (start, end));
        assert_eq!(honiara_month_bounds(end).unwrap().0, end);
    }
    #[test]
    fn empty_old_wallet_never_owes_a_later_deposit() {
        let start = at(2026, Month::January, 1);
        let mut w = RetailFeeAccountStateV1::enroll(account(), start, 30).unwrap();
        let receipts = w
            .settle_until(at(2026, Month::April, 1), true, |_| {
                Ok((1, RetailFeeScheduleV1::default()))
            })
            .unwrap();
        assert_eq!(receipts[0].collected_minor, 30);
        assert_eq!(receipts[0].waived_minor, 70);
        assert_eq!(receipts[1].collected_minor, 0);
        assert_eq!(w.balance_minor, 0);
        w.balance_minor = 1000;
        assert!(
            w.settle_until(at(2026, Month::April, 1), true, |_| Ok((
                1,
                RetailFeeScheduleV1::default()
            )))
            .unwrap()
            .is_empty()
        );
        assert_eq!(w.balance_minor, 1000);
    }
    #[test]
    fn lazy_settlement_matches_monthly_and_prorates() {
        let start = at(2026, Month::April, 16);
        let mut lazy = RetailFeeAccountStateV1::enroll(account(), start, 10000).unwrap();
        let mut eager = lazy.clone();
        let l = lazy
            .settle_until(at(2026, Month::July, 1), true, |_| {
                Ok((1, RetailFeeScheduleV1::default()))
            })
            .unwrap();
        assert_eq!(l[0].scheduled_minor, 50);
        for m in [Month::May, Month::June, Month::July] {
            eager
                .settle_until(at(2026, m, 1), true, |_| {
                    Ok((1, RetailFeeScheduleV1::default()))
                })
                .unwrap();
        }
        assert_eq!(eager, lazy);
    }
    #[test]
    fn frozen_funds_are_waived_and_not_debt() {
        let mut w =
            RetailFeeAccountStateV1::enroll(account(), at(2026, Month::April, 1), 10000).unwrap();
        let r = w
            .settle_until(at(2026, Month::May, 1), false, |_| {
                Ok((1, RetailFeeScheduleV1::default()))
            })
            .unwrap();
        assert_eq!(r[0].collected_minor, 0);
        assert_eq!(r[0].waived_minor, 100);
        assert_eq!(w.balance_minor, 10000);
    }
    #[test]
    fn policy_notice_is_calendar_based() {
        let activation = at(2026, Month::November, 1);
        assert!(validate_retail_activation(activation - RETAIL_FEE_NOTICE_MS, activation).is_ok());
        assert!(
            validate_retail_activation(activation - RETAIL_FEE_NOTICE_MS + 1, activation).is_err()
        );
        assert!(
            validate_retail_activation(activation - RETAIL_FEE_NOTICE_MS, activation + 1).is_err()
        );
    }
    #[test]
    #[ignore = "explicit maintenance capture of native retail codec fixtures"]
    fn print_native_retail_codec_fixture_v1() {
        #[derive(DeriveJsonDeserialize)]
        struct Fixture {
            request: RetailFeeQuoteRequestV1,
            assessment: RetailFeeAssessmentV1,
        }
        #[derive(DeriveJsonSerialize)]
        struct GeneratedFixture {
            request: RetailFeeQuoteRequestV1,
            assessment: RetailFeeAssessmentV1,
            request_hex: String,
            intent_hash_hex: String,
            marker: String,
            assessment_hex: String,
        }
        let fixture: Fixture = norito::json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../javascript/iroha_js/test/fixtures/retail_fee_codec_v1.json"
        )))
        .expect("shared fixture");
        let request = fixture.request;
        let mut assessment = fixture.assessment;
        assessment.intent_hash = request.intent_hash().unwrap();

        // Synthetic codec maintenance data; no ledger state or finality authority.
        assert!(!request.transfers.is_empty() && request.transfers.len() <= 1_000);
        assert!(
            request
                .transfers
                .iter()
                .all(|leg| leg.amount_minor_units > 0)
        );
        assert_eq!(assessment.account_id, request.account_id);
        assert_eq!(
            assessment.qualifying_payments,
            request.qualifying_payments()
        );
        let (start, end) = honiara_month_bounds(assessment.billing_month_start_ms).unwrap();
        assert_eq!(start, assessment.billing_month_start_ms);
        assert!(assessment.policy_revision > 0);
        assert!(assessment.qualifying_payments <= 1_000);
        assert!(
            assessment
                .payments_used_before
                .checked_add(assessment.qualifying_payments)
                .is_some()
        );
        assert!(assessment.expires_at_ms > start && assessment.expires_at_ms <= end);
        assert!(assessment.qualifying_payments != 0 || assessment.fee_minor == 0);
        for hash in [&assessment.state_commitment, &assessment.intent_hash] {
            assert!(hash.iter().any(|byte| *byte != 0) && hash[31] & 1 == 1);
        }
        let request_json = norito::json::to_vec(&request).unwrap();
        let assessment_json = norito::json::to_vec(&assessment).unwrap();
        assert!(!request_json.is_empty() && request_json.len() <= 262_144);
        assert!(!assessment_json.is_empty() && assessment_json.len() <= 4_096);
        assert_eq!(
            norito::json::from_slice::<RetailFeeQuoteRequestV1>(&request_json).unwrap(),
            request
        );
        assert_eq!(
            norito::json::from_slice::<RetailFeeAssessmentV1>(&assessment_json).unwrap(),
            assessment
        );
        let request_archive = norito::encode_canonical(&request).unwrap();
        let assessment_archive = norito::encode_canonical(&assessment).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<RetailFeeQuoteRequestV1>(&request_archive).unwrap(),
            request
        );
        assert_eq!(
            norito::decode_from_bytes::<RetailFeeAssessmentV1>(&assessment_archive).unwrap(),
            assessment
        );
        let request_hex = hex::encode(&request_archive);
        let intent_hash_hex = hex::encode(assessment.intent_hash);
        let assessment_hex = hex::encode(&assessment_archive);
        let marker = format!("iroha:retail_fee:assessment:v1:{assessment_hex}");
        assert!(!assessment_archive.is_empty() && marker.len() <= 4_096);
        let generated = GeneratedFixture {
            request,
            assessment,
            request_hex,
            intent_hash_hex,
            marker,
            assessment_hex,
        };
        println!("REQUEST_HEX={}", generated.request_hex);
        println!("INTENT_HASH_HEX={}", generated.intent_hash_hex);
        println!("ASSESSMENT_HEX={}", generated.assessment_hex);
        println!("ASSESSMENT_CANON_HEX={}", generated.assessment_hex);
        // Capture this exact serialized object for both managed fixture copies.
        println!(
            "FIXTURE_JSON={}",
            norito::json::to_json(&generated).unwrap()
        );
    }
    #[test]
    fn javascript_quote_and_marker_match_native_norito() {
        #[derive(DeriveJsonDeserialize)]
        struct Fixture {
            request: RetailFeeQuoteRequestV1,
            assessment: RetailFeeAssessmentV1,
            request_hex: String,
            intent_hash_hex: String,
            marker: String,
            assessment_hex: String,
        }
        let fixture: Fixture = norito::json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../javascript/iroha_js/test/fixtures/retail_fee_codec_v1.json"
        )))
        .expect("shared fixture");
        let assessment = fixture.assessment;
        assert_eq!(
            hex::encode(norito::encode_canonical(&fixture.request).unwrap()),
            fixture.request_hex
        );
        assert_eq!(
            hex::encode(fixture.request.intent_hash().unwrap()),
            fixture.intent_hash_hex
        );
        assert_eq!(
            assessment.intent_hash,
            fixture.request.intent_hash().unwrap()
        );
        let assessment_hex = hex::encode(norito::encode_canonical(&assessment).unwrap());
        assert_eq!(assessment_hex, fixture.assessment_hex);
        assert_eq!(
            format!("iroha:retail_fee:assessment:v1:{assessment_hex}"),
            fixture.marker
        );
    }
    #[test]
    fn same_month_reopening_preserves_allowance_and_excludes_inactive_time() {
        let mut wallet =
            RetailFeeAccountStateV1::enroll(account(), at(2026, Month::April, 1), 1000).unwrap();
        wallet
            .settle_until(at(2026, Month::April, 11), true, |_| {
                Ok((1, RetailFeeScheduleV1::default()))
            })
            .unwrap();
        wallet.payments_used = 49;
        wallet.closed_at_ms = Some(at(2026, Month::April, 11));
        wallet.reopen(at(2026, Month::April, 21), 1000).unwrap();
        assert_eq!(wallet.payments_used, 49);
        let receipts = wallet
            .settle_until(at(2026, Month::May, 1), true, |_| {
                Ok((1, RetailFeeScheduleV1::default()))
            })
            .unwrap();
        assert_eq!(receipts[0].scheduled_minor, 67);
    }
    #[test]
    fn reopening_into_lower_tier_bills_combined_active_month_only_at_boundary() {
        let schedule = RetailFeeScheduleV1::default();
        let mut wallet =
            RetailFeeAccountStateV1::enroll(account(), at(2026, Month::April, 1), 5_000_000)
                .unwrap();
        wallet.payments_used = 49;
        assert!(
            wallet
                .settle_until(at(2026, Month::April, 16), true, |_| Ok((
                    1,
                    schedule.clone()
                )))
                .unwrap()
                .is_empty()
        );
        wallet.closed_at_ms = Some(at(2026, Month::April, 16));
        wallet.balance_minor = 0;
        assert!(
            wallet
                .settle_until(at(2026, Month::April, 17), false, |_| Ok((
                    1,
                    schedule.clone()
                )))
                .unwrap()
                .is_empty()
        );
        wallet.reopen(at(2026, Month::April, 17), 1000).unwrap();
        assert_eq!(wallet.payments_used, 49);
        let paid = wallet
            .settle_until(at(2026, Month::May, 1), true, |_| Ok((1, schedule.clone())))
            .unwrap();
        assert_eq!(paid.len(), 1);
        assert_eq!(
            (
                paid[0].scheduled_minor,
                paid[0].collected_minor,
                paid[0].waived_minor
            ),
            (484, 484, 0)
        );
        assert_eq!(wallet.balance_minor, 516);
    }
    #[test]
    fn closed_calendar_boundary_waives_once_before_later_reopening() {
        let schedule = RetailFeeScheduleV1::default();
        let mut closed =
            RetailFeeAccountStateV1::enroll(account(), at(2026, Month::April, 1), 1000).unwrap();
        closed
            .settle_until(at(2026, Month::April, 16), true, |_| {
                Ok((1, schedule.clone()))
            })
            .unwrap();
        closed.closed_at_ms = Some(at(2026, Month::April, 16));
        closed.balance_minor = 0;
        let mut immediate = closed.clone();
        let first = immediate
            .settle_until(at(2026, Month::May, 1), false, |_| {
                Ok((1, schedule.clone()))
            })
            .unwrap();
        assert!(
            immediate
                .settle_until(at(2026, Month::August, 20), false, |_| Ok((
                    1,
                    schedule.clone()
                )))
                .unwrap()
                .is_empty()
        );
        let deferred = closed
            .settle_until(at(2026, Month::August, 20), false, |_| {
                Ok((1, schedule.clone()))
            })
            .unwrap();
        assert_eq!(first, deferred);
        assert_eq!(immediate, closed);
        assert_eq!(
            (
                first[0].scheduled_minor,
                first[0].collected_minor,
                first[0].waived_minor
            ),
            (50, 0, 50)
        );
        closed.reopen(at(2026, Month::August, 20), 1000).unwrap();
        assert_eq!(closed.balance_minor, 1000);
        assert_eq!(closed.payments_used, 0);
    }
    #[test]
    fn norito_schedule_and_state_roundtrip() {
        let s = RetailFeeScheduleV1::default();
        let b = norito::to_bytes(&s).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<RetailFeeScheduleV1>(&b).unwrap(),
            s
        );
        let w =
            RetailFeeAccountStateV1::enroll(account(), at(2026, Month::April, 1), 5000).unwrap();
        let b = norito::to_bytes(&w).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<RetailFeeAccountStateV1>(&b).unwrap(),
            w
        );
    }

    #[test]
    fn receipt_codec_and_hashes_bind_the_exact_payment() {
        let wallet = account();
        let period = at(2026, Month::April, 1);
        let transaction_hash = [0x11; 32];
        let receipt_id = retail_fee_receipt_id_v1(
            &wallet,
            RetailFeeReceiptKindV1::Payment,
            period,
            Some(transaction_hash),
            None,
        )
        .unwrap();
        assert_ne!(
            receipt_id,
            retail_fee_receipt_id_v1(
                &wallet,
                RetailFeeReceiptKindV1::Payment,
                period,
                Some([0x12; 32]),
                None,
            )
            .unwrap()
        );

        let receipt = RetailFeeReceiptV1 {
            wallet_id: wallet.clone(),
            sequence: 1,
            previous_receipt_hash: None,
            receipt_id,
            account_id: wallet.clone(),
            kind: RetailFeeReceiptKindV1::Payment,
            billing_month_start_ms: period,
            policy_revision: 1,
            policy_hash: [0x13; 32],
            scheduled_minor: 10,
            collected_minor: 10,
            waived_minor: 0,
            payment_count: 2,
            source_transaction_hash: Some(transaction_hash),
            effective_at_ms: None,
            recorded_at_height: 10,
            assessment: Some(RetailFeeAssessmentV1 {
                account_id: wallet.clone(),
                retail_enrolled: true,
                billing_month_start_ms: period,
                policy_revision: 1,
                payments_used_before: 49,
                qualifying_payments: 2,
                fee_minor: 10,
                state_commitment: [0x14; 32],
                intent_hash: [0x15; 32],
                expires_at_ms: period + 60_000,
            }),
        };
        let encoded = norito::to_bytes(&receipt).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<RetailFeeReceiptV1>(&encoded).unwrap(),
            receipt
        );
        let head = RetailFeeReceiptHeadV1 {
            wallet_id: wallet.clone(),
            current_account_id: wallet,
            sequence: 1,
            last_receipt_hash: Some(retail_fee_receipt_chain_hash_v1(&receipt).unwrap()),
            updated_at_height: 10,
        };
        let encoded_head = norito::to_bytes(&head).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<RetailFeeReceiptHeadV1>(&encoded_head).unwrap(),
            head
        );
        let mut altered = receipt.clone();
        altered.collected_minor -= 1;
        assert_ne!(
            retail_fee_receipt_chain_hash_v1(&receipt).unwrap(),
            retail_fee_receipt_chain_hash_v1(&altered).unwrap()
        );
    }
}

/// Native fee receipt classification; callers cannot use it to claim an exemption.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(
    tag = "kind",
    content = "value",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
#[norito_schema(name = "iroha_data_model::validation_fee::RetailFeeReceiptKindV1")]
pub enum RetailFeeReceiptKindV1 {
    /// Prorated monthly maintenance assessed at a calendar boundary or account closure.
    Maintenance,
    /// Exact fee and inclusion consumed by a successfully executed payment intent.
    Payment,
}
/// Immutable native receipt retained even when the entire scheduled charge is waived.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::validation_fee::RetailFeeReceiptV1")]
pub struct RetailFeeReceiptV1 {
    /// Original canonical wallet identity, retained across authenticated recovery.
    pub wallet_id: AccountId,
    /// Monotonic wallet receipt sequence, beginning at one.
    pub sequence: u64,
    /// Previous complete receipt chain hash; absent only at sequence one.
    pub previous_receipt_hash: Option<[u8; 32]>,
    /// Domain-separated immutable receipt identifier.
    pub receipt_id: [u8; 32],
    /// Charged canonical account.
    pub account_id: AccountId,
    /// Maintenance or executed payment assessment.
    pub kind: RetailFeeReceiptKindV1,
    /// Original earning month, preserved when physical settlement is delayed.
    pub billing_month_start_ms: u64,
    /// Original policy revision.
    pub policy_revision: u64,
    /// Hash of the complete Parliament pricing policy.
    pub policy_hash: [u8; 32],
    /// Scheduled amount in SBD cents.
    pub scheduled_minor: u64,
    /// Actually transferred amount in SBD cents.
    pub collected_minor: u64,
    /// Permanently waived amount in SBD cents.
    pub waived_minor: u64,
    /// Successful payment legs included by this receipt.
    pub payment_count: u64,
    /// Exact successful execution transaction; absent for calendar maintenance.
    pub source_transaction_hash: Option<[u8; 32]>,
    /// Logical billing boundary for maintenance; absent for a payment receipt.
    pub effective_at_ms: Option<u64>,
    /// Height physically retaining the immutable receipt.
    pub recorded_at_height: u64,
    /// Signed assessment from a successful payment, including zero-charge payments.
    pub assessment: Option<RetailFeeAssessmentV1>,
}
/// Derive the native receipt identifier from its immutable context.
///
/// # Errors
///
/// Returns an error if the immutable receipt context cannot be canonically encoded.
pub fn retail_fee_receipt_id_v1(
    account: &AccountId,
    kind: RetailFeeReceiptKindV1,
    period: u64,
    source_transaction_hash: Option<[u8; 32]>,
    effective_at_ms: Option<u64>,
) -> Result<[u8; 32], String> {
    let payload = norito::encode_canonical(&(
        account.clone(),
        kind,
        period,
        source_transaction_hash,
        effective_at_ms,
    ))
    .map_err(|e| e.to_string())?;
    Ok(domain_hash(b"iroha.retail_fee.receipt.v1", &payload))
}

/// Canonical immutable ledger storage key for a native fee receipt.
///
/// # Errors
/// Returns an error if the derived state path cannot be represented.
pub fn retail_fee_receipt_state_key_v1(receipt: &RetailFeeReceiptV1) -> Result<StatePath, String> {
    format!(
        "retail_fee_receipts_v1/{}/{}",
        hex::encode(Hash::new(receipt.account_id.to_string().as_bytes()).as_ref()),
        hex::encode(receipt.receipt_id)
    )
    .parse()
    .map_err(|error| format!("invalid native fee receipt key: {error}"))
}

/// Authenticated receipt-chain head, including the current recovered account.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::validation_fee::retail::RetailFeeReceiptHeadV1")]
pub struct RetailFeeReceiptHeadV1 {
    /// Original canonical identity shared by the entire receipt history.
    pub wallet_id: AccountId,
    /// Current canonical account after authenticated recovery, if any.
    pub current_account_id: AccountId,
    /// Number of immutable receipts in this wallet's history.
    pub sequence: u64,
    /// Last complete receipt hash; absent exactly when sequence is zero.
    pub last_receipt_hash: Option<[u8; 32]>,
    /// Block that last created a receipt or changed this account's controller identity.
    pub updated_at_height: u64,
}
/// Canonical protected receipt-head state key for a stable wallet identity.
///
/// # Errors
///
/// Returns an error if the derived receipt-head state path cannot be represented.
pub fn retail_fee_receipt_head_state_key_v1(wallet_id: &AccountId) -> Result<StatePath, String> {
    format!(
        "retail_fee_heads_v1/{}",
        hex::encode(Hash::new(wallet_id.to_string().as_bytes()).as_ref())
    )
    .parse()
    .map_err(|e| format!("invalid native receipt head key: {e}"))
}
/// Hash the complete typed receipt, including its sequence and previous hash.
///
/// # Errors
///
/// Returns an error if the complete receipt cannot be canonically encoded.
pub fn retail_fee_receipt_chain_hash_v1(receipt: &RetailFeeReceiptV1) -> Result<[u8; 32], String> {
    let bytes = norito::encode_canonical(receipt).map_err(|e| e.to_string())?;
    Ok(domain_hash(b"iroha.retail_fee.receipt_chain.v1", &bytes))
}

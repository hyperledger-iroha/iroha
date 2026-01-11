//! Deterministic UTC calendar helpers for month-based scheduling.
//!
//! These helpers avoid external time dependencies and operate purely on
//! integer arithmetic over Unix milliseconds.

use std::cmp;

use thiserror::Error;

/// Milliseconds in a UTC day.
pub const MS_PER_DAY: u64 = 86_400_000;

/// Billing period selection for month-based schedules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingPeriod {
    /// Bill for the period that just ended.
    Previous,
    /// Bill for the upcoming period.
    Next,
}

/// A concrete billing period in UTC milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthlyPeriod {
    /// Period start (inclusive) in Unix milliseconds.
    pub start_ms: u64,
    /// Period end (exclusive) in Unix milliseconds.
    pub end_ms: u64,
}

/// Errors returned by calendar helpers.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CalendarError {
    /// Anchor day must be within 1..=31.
    #[error("anchor_day must be in 1..=31")]
    InvalidAnchorDay,
    /// Anchor time must be within a single UTC day.
    #[error("anchor_time_ms must be less than {MS_PER_DAY}")]
    InvalidAnchorTime,
    /// Month must be within 1..=12.
    #[error("month must be in 1..=12")]
    InvalidMonth,
    /// Day must be within the month's day range.
    #[error("day is out of range for the month")]
    InvalidDay,
    /// Conversion overflow or negative timestamp detected.
    #[error("calendar conversion overflow")]
    Overflow,
}

/// Compute the monthly anchor at or before `ts_ms`.
///
/// The anchor uses the given `anchor_day` (clamped to the last day of the
/// month) and `anchor_time_ms` within the UTC day. If `ts_ms` is exactly on
/// the anchor, that anchor is returned.
pub fn monthly_anchor_at_or_before(
    ts_ms: u64,
    anchor_day: u8,
    anchor_time_ms: u32,
) -> Result<u64, CalendarError> {
    validate_anchor(anchor_day, anchor_time_ms)?;
    let (year, month) = year_month_from_unix_ms(ts_ms)?;
    let anchor = anchor_for_month(year, month, anchor_day, anchor_time_ms)?;
    if ts_ms < anchor {
        let (prev_year, prev_month) = prev_month(year, month)?;
        anchor_for_month(prev_year, prev_month, anchor_day, anchor_time_ms)
    } else {
        Ok(anchor)
    }
}

/// Compute the monthly anchor strictly after `ts_ms`.
///
/// If `ts_ms` is before the anchor for its month, that anchor is returned.
/// If `ts_ms` is on or after the anchor, the next month's anchor is returned.
pub fn monthly_anchor_after(
    ts_ms: u64,
    anchor_day: u8,
    anchor_time_ms: u32,
) -> Result<u64, CalendarError> {
    validate_anchor(anchor_day, anchor_time_ms)?;
    let (year, month) = year_month_from_unix_ms(ts_ms)?;
    let anchor = anchor_for_month(year, month, anchor_day, anchor_time_ms)?;
    if ts_ms < anchor {
        Ok(anchor)
    } else {
        let (next_year, next_month) = next_month(year, month)?;
        anchor_for_month(next_year, next_month, anchor_day, anchor_time_ms)
    }
}

/// Compute the billing period for a charge at `charge_at_ms`.
///
/// The period boundaries are derived from the monthly anchors defined by
/// `anchor_day` and `anchor_time_ms`.
pub fn monthly_billing_period(
    charge_at_ms: u64,
    anchor_day: u8,
    anchor_time_ms: u32,
    direction: BillingPeriod,
) -> Result<MonthlyPeriod, CalendarError> {
    let current_anchor = monthly_anchor_at_or_before(charge_at_ms, anchor_day, anchor_time_ms)?;
    match direction {
        BillingPeriod::Previous => {
            let prev_ms = current_anchor
                .checked_sub(1)
                .ok_or(CalendarError::Overflow)?;
            let start = monthly_anchor_at_or_before(prev_ms, anchor_day, anchor_time_ms)?;
            Ok(MonthlyPeriod {
                start_ms: start,
                end_ms: current_anchor,
            })
        }
        BillingPeriod::Next => {
            let end = monthly_anchor_after(current_anchor, anchor_day, anchor_time_ms)?;
            Ok(MonthlyPeriod {
                start_ms: current_anchor,
                end_ms: end,
            })
        }
    }
}

fn validate_anchor(anchor_day: u8, anchor_time_ms: u32) -> Result<(), CalendarError> {
    if !(1..=31).contains(&anchor_day) {
        return Err(CalendarError::InvalidAnchorDay);
    }
    if u64::from(anchor_time_ms) >= MS_PER_DAY {
        return Err(CalendarError::InvalidAnchorTime);
    }
    Ok(())
}

fn year_month_from_unix_ms(ts_ms: u64) -> Result<(i32, u8), CalendarError> {
    let days = i64::try_from(ts_ms / MS_PER_DAY).map_err(|_| CalendarError::Overflow)?;
    let (year, month, _day) = civil_from_days(days)?;
    Ok((year, month))
}

fn anchor_for_month(
    year: i32,
    month: u8,
    anchor_day: u8,
    anchor_time_ms: u32,
) -> Result<u64, CalendarError> {
    let dim = days_in_month(year, month)?;
    let day = cmp::min(anchor_day, dim);
    let days = days_from_civil(year, month, day)?;
    let ms = i128::from(days) * i128::from(MS_PER_DAY) + i128::from(anchor_time_ms);
    if ms < 0 || ms > i128::from(u64::MAX) {
        return Err(CalendarError::Overflow);
    }
    Ok(ms as u64)
}

fn prev_month(year: i32, month: u8) -> Result<(i32, u8), CalendarError> {
    if !(1..=12).contains(&month) {
        return Err(CalendarError::InvalidMonth);
    }
    if month == 1 {
        let year = year.checked_sub(1).ok_or(CalendarError::Overflow)?;
        Ok((year, 12))
    } else {
        Ok((year, month - 1))
    }
}

fn next_month(year: i32, month: u8) -> Result<(i32, u8), CalendarError> {
    if !(1..=12).contains(&month) {
        return Err(CalendarError::InvalidMonth);
    }
    if month == 12 {
        let year = year.checked_add(1).ok_or(CalendarError::Overflow)?;
        Ok((year, 1))
    } else {
        Ok((year, month + 1))
    }
}

fn days_in_month(year: i32, month: u8) -> Result<u8, CalendarError> {
    if !(1..=12).contains(&month) {
        return Err(CalendarError::InvalidMonth);
    }
    let is_leap = is_leap_year(year);
    let dim = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap {
                29
            } else {
                28
            }
        }
        _ => return Err(CalendarError::InvalidMonth),
    };
    Ok(dim)
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0) && ((year % 100 != 0) || (year % 400 == 0))
}

// Convert days since Unix epoch (1970-01-01) to Y-M-D.
fn civil_from_days(days: i64) -> Result<(i32, u8, u8), CalendarError> {
    // Algorithm adapted from Howard Hinnant's date algorithms.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    let year = i32::try_from(year).map_err(|_| CalendarError::Overflow)?;
    let month = u8::try_from(m).map_err(|_| CalendarError::Overflow)?;
    let day = u8::try_from(d).map_err(|_| CalendarError::Overflow)?;
    Ok((year, month, day))
}

// Convert Y-M-D to days since Unix epoch (1970-01-01).
fn days_from_civil(year: i32, month: u8, day: u8) -> Result<i64, CalendarError> {
    let dim = days_in_month(year, month)?;
    if day == 0 || day > dim {
        return Err(CalendarError::InvalidDay);
    }
    let y = i64::from(year) - i64::from(month <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = i64::from(month) + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Ok(days)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unix_ms(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8, ms: u16) -> u64 {
        let days = days_from_civil(year, month, day).expect("valid date");
        let day_ms = u64::try_from(days).expect("positive days") * MS_PER_DAY;
        day_ms
            + u64::from(hour) * 3_600_000
            + u64::from(minute) * 60_000
            + u64::from(second) * 1_000
            + u64::from(ms)
    }

    #[test]
    fn anchor_at_or_before_returns_current_anchor() {
        let ts = unix_ms(2024, 3, 15, 12, 0, 0, 0);
        let anchor = monthly_anchor_at_or_before(ts, 1, 0).expect("anchor");
        assert_eq!(anchor, unix_ms(2024, 3, 1, 0, 0, 0, 0));
    }

    #[test]
    fn anchor_at_or_before_steps_back_when_before_anchor() {
        let anchor = unix_ms(2024, 3, 1, 0, 0, 0, 0);
        let ts = anchor - 1;
        let anchor = monthly_anchor_at_or_before(ts, 1, 0).expect("anchor");
        assert_eq!(anchor, unix_ms(2024, 2, 1, 0, 0, 0, 0));
    }

    #[test]
    fn anchor_after_returns_next_on_anchor() {
        let ts = unix_ms(2024, 3, 1, 0, 0, 0, 0);
        let anchor = monthly_anchor_after(ts, 1, 0).expect("anchor");
        assert_eq!(anchor, unix_ms(2024, 4, 1, 0, 0, 0, 0));
    }

    #[test]
    fn anchor_after_returns_current_when_before_anchor() {
        let ts = unix_ms(2024, 3, 10, 0, 0, 0, 0);
        let anchor = monthly_anchor_after(ts, 20, 0).expect("anchor");
        assert_eq!(anchor, unix_ms(2024, 3, 20, 0, 0, 0, 0));
    }

    #[test]
    fn monthly_period_previous_uses_last_anchor() {
        let charge_at = unix_ms(2024, 4, 1, 0, 0, 0, 0);
        let period =
            monthly_billing_period(charge_at, 1, 0, BillingPeriod::Previous).expect("period");
        assert_eq!(period.start_ms, unix_ms(2024, 3, 1, 0, 0, 0, 0));
        assert_eq!(period.end_ms, unix_ms(2024, 4, 1, 0, 0, 0, 0));
    }

    #[test]
    fn monthly_period_next_uses_next_anchor() {
        let charge_at = unix_ms(2024, 4, 1, 0, 0, 0, 0);
        let period = monthly_billing_period(charge_at, 1, 0, BillingPeriod::Next).expect("period");
        assert_eq!(period.start_ms, unix_ms(2024, 4, 1, 0, 0, 0, 0));
        assert_eq!(period.end_ms, unix_ms(2024, 5, 1, 0, 0, 0, 0));
    }

    #[test]
    fn anchor_day_clamps_to_last_day_of_month() {
        let ts = unix_ms(2024, 2, 29, 12, 0, 0, 0);
        let anchor = monthly_anchor_at_or_before(ts, 31, 0).expect("anchor");
        assert_eq!(anchor, unix_ms(2024, 2, 29, 0, 0, 0, 0));
    }

    #[test]
    fn invalid_anchor_time_is_rejected() {
        let ts = unix_ms(2024, 1, 1, 0, 0, 0, 0);
        let err = monthly_anchor_at_or_before(ts, 1, MS_PER_DAY as u32);
        assert_eq!(err, Err(CalendarError::InvalidAnchorTime));
    }
}

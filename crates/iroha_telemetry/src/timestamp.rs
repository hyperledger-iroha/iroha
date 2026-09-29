//! RFC 3339 timestamps for exported telemetry records.

use std::fmt::Write as _;

use time::OffsetDateTime;

/// Current UTC time as RFC 3339 text for the `ts` field of telemetry records.
pub fn rfc3339_utc_now() -> String {
    format_rfc3339_utc(OffsetDateTime::now_utc())
}

/// Format `at` (interpreted in UTC) with the minimal SI fraction.
///
/// Collectors parse this exact shape: seconds precision, then `.mmm`, `.uuuuuu`
/// or `.nnnnnnnnn` only when the sub-second part needs it, and a numeric
/// `+00:00` offset rather than `Z`.
fn format_rfc3339_utc(at: OffsetDateTime) -> String {
    let at = at.to_offset(time::UtcOffset::UTC);
    let mut out = String::with_capacity(35);
    // Writing into a `String` cannot fail.
    let _ = write!(
        out,
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second()
    );
    let nanos = at.nanosecond();
    if nanos != 0 {
        if nanos.is_multiple_of(1_000_000) {
            let _ = write!(out, ".{:03}", nanos / 1_000_000);
        } else if nanos.is_multiple_of(1_000) {
            let _ = write!(out, ".{:06}", nanos / 1_000);
        } else {
            let _ = write!(out, ".{nanos:09}");
        }
    }
    out.push_str("+00:00");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(unix_nanos: i128) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp_nanos(unix_nanos).expect("valid timestamp")
    }

    const BASE: i128 = 1_700_000_000 * 1_000_000_000;

    #[test]
    fn whole_seconds_have_no_fraction() {
        assert_eq!(format_rfc3339_utc(at(BASE)), "2023-11-14T22:13:20+00:00");
    }

    #[test]
    fn fraction_uses_the_shortest_si_width() {
        assert_eq!(
            format_rfc3339_utc(at(BASE + 120_000_000)),
            "2023-11-14T22:13:20.120+00:00"
        );
        assert_eq!(
            format_rfc3339_utc(at(BASE + 123_456_000)),
            "2023-11-14T22:13:20.123456+00:00"
        );
        assert_eq!(
            format_rfc3339_utc(at(BASE + 123_456_789)),
            "2023-11-14T22:13:20.123456789+00:00"
        );
        assert_eq!(
            format_rfc3339_utc(at(BASE + 1)),
            "2023-11-14T22:13:20.000000001+00:00"
        );
    }

    #[test]
    fn non_utc_offsets_are_normalized() {
        let local = at(BASE).to_offset(time::UtcOffset::from_hms(9, 0, 0).expect("offset"));
        assert_eq!(format_rfc3339_utc(local), "2023-11-14T22:13:20+00:00");
    }

    #[test]
    fn now_is_well_formed() {
        let now = rfc3339_utc_now();
        assert!(now.ends_with("+00:00"));
        assert_eq!(&now[4..5], "-");
        assert_eq!(&now[10..11], "T");
    }
}

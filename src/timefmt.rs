//! Minimal UTC time helpers (no chrono dependency).
//! Uses Howard Hinnant's civil-date algorithms.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// epoch seconds -> (year, month, day, hour, min, sec) in UTC
fn civil_from_epoch(epoch: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (epoch / 86_400) as i64;
    let secs = epoch % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (
        y,
        m,
        d,
        (secs / 3600) as u32,
        ((secs % 3600) / 60) as u32,
        (secs % 60) as u32,
    )
}

/// (year, month, day) -> days since 1970-01-01
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// ISO 8601 UTC, e.g. "2026-07-12T09:03:22Z"
pub fn iso_utc(epoch: u64) -> String {
    let (y, mo, d, h, mi, s) = civil_from_epoch(epoch);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, mi, s)
}

/// Compact form for filenames, e.g. "20260712-090322"
pub fn compact_ts(epoch: u64) -> String {
    let (y, mo, d, h, mi, s) = civil_from_epoch(epoch);
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", y, mo, d, h, mi, s)
}

/// Parse "YYYY-MM-DDTHH:MM:SS(.frac)?(Z|+00:00)?" -> epoch seconds (UTC).
/// Non-UTC offsets return None (rare in transcripts; callers treat as unknown).
pub fn parse_iso_to_epoch(s: &str) -> Option<u64> {
    let s = s.trim();
    let (date, time) = s.split_once('T')?;
    let mut dp = date.split('-');
    let y: i64 = dp.next()?.parse().ok()?;
    let mo: u32 = dp.next()?.parse().ok()?;
    let d: u32 = dp.next()?.parse().ok()?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    // strip timezone suffix
    let time = time.trim_end();
    let time = if let Some(t) = time.strip_suffix('Z') {
        t
    } else if let Some(t) = time.strip_suffix("+00:00") {
        t
    } else if time.contains('+') || time.matches('-').count() > 0 {
        return None; // non-UTC offset
    } else {
        time
    };
    // strip fractional seconds
    let time = time.split('.').next()?;
    let mut tp = time.split(':');
    let h: u64 = tp.next()?.parse().ok()?;
    let mi: u64 = tp.next()?.parse().ok()?;
    let sec: u64 = tp.next().unwrap_or("0").parse().ok()?;
    if h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    let days = days_from_civil(y, mo, d);
    if days < 0 {
        return None;
    }
    Some(days as u64 * 86_400 + h * 3600 + mi * 60 + sec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_zero() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn known_date() {
        // 2026-07-12 00:00:00 UTC
        let e = parse_iso_to_epoch("2026-07-12T00:00:00Z").unwrap();
        assert_eq!(iso_utc(e), "2026-07-12T00:00:00Z");
    }

    #[test]
    fn roundtrip_with_millis() {
        let e = parse_iso_to_epoch("2025-12-31T23:59:59.123Z").unwrap();
        assert_eq!(iso_utc(e), "2025-12-31T23:59:59Z");
    }

    #[test]
    fn rejects_offset() {
        assert!(parse_iso_to_epoch("2026-01-01T00:00:00+02:00").is_none());
    }

    #[test]
    fn compact() {
        let e = parse_iso_to_epoch("2026-07-12T09:03:22Z").unwrap();
        assert_eq!(compact_ts(e), "20260712-090322");
    }
}

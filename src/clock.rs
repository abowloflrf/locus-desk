use chrono::{DateTime, Days, LocalResult, NaiveDate, SecondsFormat, TimeDelta, TimeZone, Utc};
use chrono_tz::Tz;

use crate::error::{AppError, AppResult};

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[cfg(test)]
#[derive(Debug)]
pub struct FixedClock {
    now: DateTime<Utc>,
}

#[cfg(test)]
impl FixedClock {
    pub const fn new(now: DateTime<Utc>) -> Self {
        Self { now }
    }
}

#[cfg(test)]
impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.now
    }
}

pub fn timestamp_millis(clock: &dyn Clock) -> i64 {
    clock.now().timestamp_millis()
}

pub fn format_timestamp(value: i64) -> AppResult<String> {
    DateTime::<Utc>::from_timestamp_millis(value)
        .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Millis, true))
        .ok_or_else(|| AppError::Internal(format!("invalid stored timestamp: {value}")))
}

pub fn today(now: DateTime<Utc>, timezone: Tz) -> NaiveDate {
    now.with_timezone(&timezone).date_naive()
}

pub fn local_day_bounds(date: NaiveDate, timezone: Tz) -> AppResult<(i64, i64)> {
    let next_date = date
        .checked_add_days(Days::new(1))
        .ok_or_else(|| AppError::Internal("local date overflow".to_owned()))?;
    let start = first_valid_instant(date, timezone)?;
    let end = first_valid_instant(next_date, timezone)?;
    Ok((start.timestamp_millis(), end.timestamp_millis()))
}

fn first_valid_instant(date: NaiveDate, timezone: Tz) -> AppResult<DateTime<Utc>> {
    let local = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::Internal("invalid local midnight".to_owned()))?;
    // Midnight can be skipped by an offset transition, and an entire local date can be skipped.
    for minute in 0..=(3 * 24 * 60) {
        let candidate = local
            .checked_add_signed(TimeDelta::minutes(minute))
            .ok_or_else(|| AppError::Internal("local date overflow".to_owned()))?;
        match timezone.from_local_datetime(&candidate) {
            LocalResult::Single(value) => return Ok(value.with_timezone(&Utc)),
            LocalResult::Ambiguous(first, _) => return Ok(first.with_timezone(&Utc)),
            LocalResult::None => {}
        }
    }
    Err(AppError::Internal(format!(
        "could not resolve a local day boundary for {date} in {timezone}"
    )))
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate, Utc};

    use super::{local_day_bounds, today};

    #[test]
    fn local_day_bounds_follow_dst_and_calendar_boundaries() {
        let new_york = "America/New_York".parse().unwrap();
        let spring = NaiveDate::from_ymd_opt(2026, 3, 8).unwrap();
        let autumn = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let (spring_start, spring_end) = local_day_bounds(spring, new_york).unwrap();
        let (autumn_start, autumn_end) = local_day_bounds(autumn, new_york).unwrap();
        assert_eq!(spring_end - spring_start, 23 * 60 * 60 * 1_000);
        assert_eq!(autumn_end - autumn_start, 25 * 60 * 60 * 1_000);

        let utc = DateTime::parse_from_rfc3339("2026-12-31T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let kiritimati = "Pacific/Kiritimati".parse().unwrap();
        assert_eq!(today(utc, kiritimati).to_string(), "2027-01-01");
    }
}

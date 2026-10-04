//! The timestamp a run directory is named for.
//!
//! FR-2.6c puts `.bolt-<iso8601>` at the run's base, in the filesystem-safe
//! form and not the strict one, so a directory listing sorts by run.
//!
//! UTC, and computed here instead of taken from a crate. A local offset
//! cannot be had from the standard library, and the alternative was a
//! dependency whose only job is one filename. Whether a run directory should
//! carry a local offset belongs with FR-2.6's other questions in `runner/10`.

use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds in a day.
const DAY: u64 = 86_400;

/// `YYYY-MM-DDTHH-MM-SSZ` for `at`, colons replaced so a path can hold it.
///
/// A time before the epoch is not representable and returns the epoch itself,
/// which cannot arise from a run and does not justify a refusal.
#[must_use]
pub fn iso8601(at: SystemTime) -> String {
    let seconds = at.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let (year, month, day) = civil_from_days(seconds / DAY);
    let rest = seconds % DAY;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}-{:02}-{:02}Z",
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60,
    )
}

/// When a default run directory named `name` was started, or `None` where the
/// name is not one.
///
/// Three spellings are read. This build writes `.bolt-<stamp>Z-<pid>`, an
/// earlier one wrote `.bolt-<stamp>Z`, and the Go build wrote the local offset,
/// `.bolt-<stamp>-07-00`, sometimes followed by `-<pid>`. All of them are run
/// directories a dogfooding tree accumulates, so FR-13.6 reads all of them.
#[must_use]
pub fn started(name: &str) -> Option<SystemTime> {
    let rest = name.strip_prefix(".bolt-")?;
    let (stamp, zone) = (rest.get(..19)?, rest.get(19..)?);
    let field = |at: usize, width: usize| -> Option<u64> {
        let text = stamp.get(at..at + width)?;
        text.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| text.parse().ok())?
    };
    let separators = [(4, b'-'), (7, b'-'), (10, b'T'), (13, b'-'), (16, b'-')];
    if separators
        .iter()
        .any(|&(at, byte)| stamp.as_bytes()[at] != byte)
    {
        return None;
    }
    let (year, month, day) = (field(0, 4)?, field(5, 2)?, field(8, 2)?);
    let (hour, minute, second) = (field(11, 2)?, field(14, 2)?, field(17, 2)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }

    let local = days_from_civil(year, month, day)? * DAY + hour * 3600 + minute * 60 + second;
    let utc = apply_zone(local, zone)?;
    Some(UNIX_EPOCH + std::time::Duration::from_secs(utc))
}

/// `local` seconds made UTC by the zone written after the stamp.
///
/// `Z` or `±HH-MM`, either optionally followed by `-<pid>`.
fn apply_zone(local: u64, zone: &str) -> Option<u64> {
    let pid = |tail: &str| {
        tail.is_empty()
            || tail
                .strip_prefix('-')
                .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
    };
    if let Some(tail) = zone.strip_prefix('Z') {
        return pid(tail).then_some(local);
    }
    let sign = zone.as_bytes().first().copied()?;
    let (hours, minutes, tail) = (zone.get(1..3)?, zone.get(4..6)?, zone.get(6..)?);
    if zone.as_bytes().get(3) != Some(&b'-') || !pid(tail) {
        return None;
    }
    let offset = hours.parse::<u64>().ok()? * 3600 + minutes.parse::<u64>().ok()? * 60;
    match sign {
        b'+' => local.checked_sub(offset),
        b'-' => local.checked_add(offset),
        _ => None,
    }
}

/// Days from 1970-01-01 to a civil date, the inverse of [`civil_from_days`].
///
/// `None` before the epoch, which no run directory can be named for.
fn days_from_civil(year: u64, month: u64, day: u64) -> Option<u64> {
    let year = if month <= 2 {
        year.checked_sub(1)?
    } else {
        year
    };
    let era = year / 400;
    let year_of_era = year - era * 400;
    let month_prime = (month + 9) % 12;
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    (era * 146_097 + day_of_era).checked_sub(719_468)
}

/// The civil date `days` after 1970-01-01, by Howard Hinnant's algorithm.
///
/// It shifts the epoch to 0000-03-01 so that February, and therefore the leap
/// day, falls at the end of the year and needs no special case.
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;

    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);

    (year, month, day)
}

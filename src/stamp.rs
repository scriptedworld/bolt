//! The timestamp a run directory is named for.
//!
//! FR-2.6c puts `.bolt-<iso8601>` at the run's base, in the filesystem-safe
//! form rather than the strict one, so a directory listing sorts by run.
//!
//! UTC, and written out here rather than taken from a crate. A local offset
//! cannot be had from the standard library, and the alternative was a
//! dependency whose only job is one filename. Whether a run directory should
//! carry a local offset belongs with FR-2.6's other questions in `runner/10`.

use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds in a day.
const DAY: u64 = 86_400;

/// `YYYY-MM-DDTHH-MM-SSZ` for `at`, colons replaced so a path can hold it.
///
/// A time before the epoch is not representable and returns the epoch itself,
/// which cannot arise from a run and is not worth a refusal.
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

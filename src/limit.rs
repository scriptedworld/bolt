//! Time limits, and how long is left of one.
//!
//! FR-4.11 gives a task and a run each an optional limit, so unset means a tool
//! is allowed to finish. FR-4.11e settles the spelling, and this module is the
//! whole of what reads it: a decimal followed by `s`, `m` or `h`.

use std::time::{Duration, Instant};

/// The reason `kind` a passed limit produces, by FR-7.9.
///
/// One kind for both limits, because a consumer telling a slow task from a slow
/// run reads the message, and FR-7.10's distinction is between a task that could
/// not execute and one that executed and failed. A timeout is the second of
/// those whichever limit fired.
pub const KIND: &str = "time-limit";

/// Read a limit, by FR-4.11e.
///
/// A decimal followed by `s`, `m` or `h`: `30s`, `1.5m`, `2h`. Readable in a
/// jig, and a grammar any language parses in a few lines, which matters because
/// this build is the reference one and is expected to be translated.
///
/// Deliberately narrower than `f64::from_str`, which would take `1e3s`, `+5s`
/// and `infs`. Those are things a person did not mean to write, and accepting
/// them would make the grammar something a second implementation has to
/// discover rather than read.
///
/// Returns `None` for anything else, which FR-4.11e makes a refusal before any
/// task executes rather than a task that fails partway through a gate.
#[must_use]
pub fn parse(value: &str) -> Option<Duration> {
    let (quantity, per_unit) = if let Some(rest) = value.strip_suffix('s') {
        (rest, 1.0)
    } else if let Some(rest) = value.strip_suffix('m') {
        (rest, 60.0)
    } else {
        (value.strip_suffix('h')?, 3600.0)
    };

    Duration::try_from_secs_f64(decimal(quantity)? * per_unit).ok()
}

/// A decimal: ASCII digits and at most one point, with at least one digit.
fn decimal(text: &str) -> Option<f64> {
    let mut points = 0;
    let mut digits = 0;
    for character in text.chars() {
        match character {
            '.' => points += 1,
            _ if character.is_ascii_digit() => digits += 1,
            _ => return None,
        }
    }
    if points > 1 || digits == 0 {
        return None;
    }
    text.parse().ok()
}

/// When a limit set now would run out, or `None` where none is set.
#[must_use]
pub fn deadline(limit: Option<Duration>, from: Instant) -> Option<Instant> {
    limit.map(|limit| from + limit)
}

/// Whichever of two deadlines comes first, taking a set one over none.
#[must_use]
pub fn soonest(one: Option<Instant>, other: Option<Instant>) -> Option<Instant> {
    match (one, other) {
        (Some(one), Some(other)) => Some(one.min(other)),
        (set, None) | (None, set) => set,
    }
}

/// Whether `deadline` has already passed at `now`.
#[must_use]
pub fn passed(deadline: Option<Instant>, now: Instant) -> bool {
    deadline.is_some_and(|deadline| now >= deadline)
}

//! Removing old default run directories, when a run is asked to, by FR-13.6.
//!
//! Nothing here runs unless `--remove-old-runs` was named. What it may remove is
//! narrow on purpose, because it deletes files: a real directory directly under
//! the base, named the way FR-2.6c names a default run directory, stamped more
//! than seven days before this run started. A directory named with
//! `--output-dir` has no stamp in its name unless its caller wrote one, and a
//! symlink is never followed or removed.

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::{Error, stamp};

/// How old a run directory has to be before it is removed.
pub const MAX_AGE: Duration = Duration::from_hours(7 * 24);

/// Remove every default run directory under `base` older than [`MAX_AGE`] at
/// `now`, leaving `own` alone, and return their names in sorted order.
///
/// # Errors
///
/// [`Error::Io`] when the base cannot be listed or a directory cannot be
/// removed. The run is refused, so a directory that would not go is named
/// instead of being skipped silently.
pub fn remove_old_runs(base: &Path, own: &Path, now: SystemTime) -> Result<Vec<String>, Error> {
    let io = |path: &Path, source: &std::io::Error| Error::Io {
        path: path.to_path_buf(),
        reason: source.to_string(),
    };
    let mut removed = Vec::new();
    for entry in fs::read_dir(base).map_err(|source| io(base, &source))? {
        let path = entry.map_err(|source| io(base, &source))?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let old = stamp::started(name)
            .and_then(|started| now.duration_since(started).ok())
            .is_some_and(|age| age > MAX_AGE);
        let directory = fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_dir());
        if old && directory && path != own {
            fs::remove_dir_all(&path).map_err(|source| io(&path, &source))?;
            removed.push(name.to_owned());
        }
    }
    removed.sort();
    Ok(removed)
}

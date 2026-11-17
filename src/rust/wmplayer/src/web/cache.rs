//! The downloaded tracks: ${XDG_CACHE_HOME:-~/.cache}/wmplayer/web/KEY.EXT,
//! and KEY.EXT.part while one is downloading. A track that has not been
//! played for a week is removed.

use std::fs::{self, File};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const KEEP: Duration = Duration::from_secs(7 * 24 * 3600);

pub fn dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join("wmplayer").join("web");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// The finished file for `key`, and its extension.
pub fn find(key: &str) -> Option<(PathBuf, String)> {
    let prefix = format!("{key}.");
    fs::read_dir(dir()).ok()?.flatten().find_map(|e| {
        let name = e.file_name().into_string().ok()?;
        let ext = name.strip_prefix(&prefix)?;
        (!ext.is_empty() && !ext.contains('.')).then(|| (e.path(), ext.to_owned()))
    })
}

pub fn path(key: &str, ext: &str) -> PathBuf {
    dir().join(format!("{key}.{ext}"))
}

pub fn part_path(key: &str, ext: &str) -> PathBuf {
    dir().join(format!("{key}.{ext}.part"))
}

/// Marks the file as played now, for prune().
pub fn touch(path: &PathBuf) {
    if let Ok(f) = File::options().append(true).open(path) {
        let _ = f.set_modified(SystemTime::now());
    }
}

/// Removes what was left half downloaded, and what has not been played for
/// a week. For the start, when nothing is downloading.
pub fn prune() {
    let Ok(entries) = fs::read_dir(dir()) else { return };
    for e in entries.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > KEEP);
        let part = e.file_name().to_string_lossy().ends_with(".part");
        if old || part {
            let _ = fs::remove_file(e.path());
        }
    }
}

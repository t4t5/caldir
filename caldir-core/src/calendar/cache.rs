//! Process-wide cache for parsed calendar events, keyed by file path + mtime.
//!
//! Aimed at long-running consumers of caldir-core (e.g. desktop GUIs) where
//! `Calendar::events()` is invoked repeatedly across a session — for rendering,
//! searching, recurrence expansion, and edit lookups. Without caching, each
//! call reads and parses every `.ics` file in the calendar directory; for a
//! real-world calendar with thousands of events that's tens of megabytes of
//! file I/O and parsing per call.
//!
//! For the one-shot `caldir` CLI this is effectively a no-op: each invocation
//! is a fresh process and typically calls `events()` once, so the cache starts
//! and ends empty. The cost it adds (a `LazyLock<Mutex>` and an extra clone on
//! hits) is negligible there, and the win for GUI hosts is large.
//!
//! The cache stores one entry per `.ics` file. On lookup we still `read_dir`
//! and `stat` each file (cheap) but skip parsing whenever the cached entry's
//! mtime matches. Files added since the last call get parsed; files removed
//! get pruned from the cache.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use crate::calendar::event::CalendarEvent;
use crate::error::CalDirResult;

/// Per-file cache entry. `event` is `None` when parsing failed — cached so we
/// don't re-attempt a malformed file every call. Re-parsed when mtime changes.
struct CacheEntry {
    mtime: Option<SystemTime>,
    event: Option<CalendarEvent>,
}

type DirCache = HashMap<PathBuf, CacheEntry>;

static CACHE: LazyLock<Mutex<HashMap<PathBuf, DirCache>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Read the calendar directory and return parsed events, using cached parses
/// where the file's mtime hasn't changed since we last saw it.
pub(super) fn cached_events_for_dir(dir: &Path) -> CalDirResult<Vec<CalendarEvent>> {
    // Phase 1: enumerate `.ics` files and grab mtimes (one stat each).
    let mut entries: Vec<(PathBuf, Option<SystemTime>)> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if !path.extension().is_some_and(|e| e == "ics") {
            continue;
        }
        let mtime = entry.metadata().ok().and_then(|m| m.modified().ok());
        entries.push((path, mtime));
    }

    // Phase 2: serve from cache, parse on miss. We hold the lock through
    // file I/O — fine in practice because callers are sequential per process,
    // and parses only run on cache misses (typically zero after warm-up).
    let mut cache = CACHE.lock().unwrap();
    let dir_cache = cache.entry(dir.to_path_buf()).or_default();

    let mut events = Vec::with_capacity(entries.len());
    let mut present: HashSet<PathBuf> = HashSet::with_capacity(entries.len());

    for (path, mtime) in entries {
        present.insert(path.clone());

        let needs_parse = match dir_cache.get(&path) {
            Some(entry) => entry.mtime != mtime,
            None => true,
        };

        if needs_parse {
            let event = CalendarEvent::from_file(path.clone()).ok();
            if let Some(ref e) = event {
                events.push(e.clone());
            }
            dir_cache.insert(path, CacheEntry { mtime, event });
        } else if let Some(entry) = dir_cache.get(&path)
            && let Some(event) = &entry.event
        {
            events.push(event.clone());
        }
    }

    // Phase 3: drop entries for files that no longer exist.
    dir_cache.retain(|p, _| present.contains(p));

    Ok(events)
}

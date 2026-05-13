use caldir_core::diff::{CalendarDiff, DiffKind};
use owo_colors::OwoColorize;

/// Number of pending deletions that triggers the safeguard.
const MASS_DELETE_THRESHOLD: usize = 10;

/// Refuses a push that would delete a large number of remote events. Prints a
/// warning and returns `false` when blocked; callers should `continue` past
/// the calendar in that case.
pub fn allow_mass_delete(diff: &CalendarDiff, force: bool) -> bool {
    if force {
        return true;
    }
    let delete_count = diff
        .to_push
        .iter()
        .filter(|d| d.kind == DiffKind::Delete)
        .count();
    if delete_count < MASS_DELETE_THRESHOLD {
        return true;
    }
    println!(
        "   {}",
        "You are about to delete many events! If you're sure, re-run with --force. Otherwise, run \"caldir discard\" to restore from remote.".red()
    );
    false
}

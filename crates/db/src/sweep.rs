/// Recover or clear the temporaries an interrupted write left behind.
///
/// Run when a vault opens. See [`crate::atomic`] for the write protocol: a
/// `.part` is incomplete by definition, while a `.ready` holds a whole note
/// whose publish did not happen.
///
/// Recovery is deliberately conservative. A `.ready` is published only when
/// nothing would be overwritten, because between the crash and this sweep the
/// target may have been replaced by something the user cares about more — a
/// `git pull`, a sync from another device. When both exist and differ, the
/// `.ready` is left alone and reported rather than silently winning.
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::atomic::{PART, READY};
use crate::sync::is_ignored_dir;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SweepReport {
    /// Complete notes put back, because their target was gone.
    pub recovered: Vec<String>,
    /// Incomplete or superseded temporaries deleted.
    pub discarded: usize,
    /// Complete notes that would have overwritten a different file. Left in
    /// place for the user to deal with; the caller should say so out loud.
    pub conflicted: Vec<String>,
}

impl SweepReport {
    pub fn is_empty(&self) -> bool {
        self.recovered.is_empty() && self.discarded == 0 && self.conflicted.is_empty()
    }
}

const PART_SUFFIX: &str = ".part";
const READY_SUFFIX: &str = ".ready";

/// Split `.<name>.<id>.<phase>` back into the target name and the phase.
pub fn parse_temp_name(name: &str) -> Option<(String, &'static str)> {
    let rest = name.strip_prefix('.')?;
    let (rest, phase) = match rest {
        _ if rest.ends_with(READY_SUFFIX) => (&rest[..rest.len() - READY_SUFFIX.len()], READY),
        _ if rest.ends_with(PART_SUFFIX) => (&rest[..rest.len() - PART_SUFFIX.len()], PART),
        _ => return None,
    };
    let (target, id) = rest.rsplit_once('.')?;
    if target.is_empty() || id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some((target.to_string(), phase))
}

/// True for the single-phase name written before the protocol had phases. Their
/// completeness is unknowable, so they are only ever discarded.
fn is_legacy_temp(name: &str) -> bool {
    let Some(rest) = name.strip_prefix('.').and_then(|r| r.strip_suffix(".tmp")) else {
        return false;
    };
    match rest.rsplit_once('.') {
        Some((target, id)) => {
            !target.is_empty() && id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())
        }
        None => false,
    }
}

fn older_than(path: &Path, min_age: Duration, now: SystemTime) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|modified| now.duration_since(modified).unwrap_or(Duration::ZERO) >= min_age)
        .unwrap_or(false)
}

/// Sweep a vault. `min_age` keeps a write that is still in flight — in this
/// process or another copy of the app — from being disturbed.
pub fn sweep(root: &Path, min_age: Duration) -> SweepReport {
    let mut report = SweepReport::default();
    walk(root, min_age, SystemTime::now(), &mut report);
    report
}

fn walk(dir: &Path, min_age: Duration, now: SystemTime, report: &mut SweepReport) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();

        if file_type.is_dir() {
            if !is_ignored_dir(&name) {
                walk(&path, min_age, now, report);
            }
            continue;
        }
        if !file_type.is_file() || !older_than(&path, min_age, now) {
            continue;
        }

        if is_legacy_temp(&name) {
            if std::fs::remove_file(&path).is_ok() {
                report.discarded += 1;
            }
            continue;
        }

        let Some((target_name, phase)) = parse_temp_name(&name) else {
            continue;
        };

        if phase == PART {
            // Interrupted mid-write; its content is a prefix of a note, not a note.
            if std::fs::remove_file(&path).is_ok() {
                report.discarded += 1;
            }
            continue;
        }

        resolve_ready(dir, &path, &target_name, report);
    }
}

fn resolve_ready(dir: &Path, ready: &Path, target_name: &str, report: &mut SweepReport) {
    let target: PathBuf = dir.join(target_name);

    if !target.exists() {
        // The publish rename never happened, or happened after the delete on
        // Android. Nothing is at risk, so put the note back.
        if std::fs::rename(ready, &target).is_ok() {
            report.recovered.push(target.to_string_lossy().to_string());
        }
        return;
    }

    let same = match (std::fs::read(ready), std::fs::read(&target)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if same {
        // The publish did land; this is just an unremoved leftover.
        if std::fs::remove_file(ready).is_ok() {
            report.discarded += 1;
        }
    } else {
        report.conflicted.push(ready.to_string_lossy().to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const ID: &str = "0123456789abcdef0123456789abcdef";
    const NOW: Duration = Duration::ZERO;

    fn temp(dir: &Path, target: &str, phase: &str, body: &str) -> PathBuf {
        let p = dir.join(format!(".{target}.{ID}.{phase}"));
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn suffix_constants_track_the_phase_names() {
        // The suffixes are spelled out so parsing needs no allocation; this keeps
        // them honest if a phase is ever renamed.
        assert_eq!(PART_SUFFIX, format!(".{PART}"));
        assert_eq!(READY_SUFFIX, format!(".{READY}"));
    }

    #[test]
    fn names_round_trip() {
        assert_eq!(
            parse_temp_name(&format!(".inbox.org.{ID}.ready")),
            Some(("inbox.org".to_string(), READY))
        );
        assert_eq!(
            parse_temp_name(&format!(".inbox.org.{ID}.part")),
            Some(("inbox.org".to_string(), PART))
        );
    }

    #[test]
    fn other_peoples_files_are_not_ours_to_touch() {
        for name in [
            "inbox.org",
            "notes.part",                       // no leading dot, no id
            ".vim.ready",                       // no id
            &format!(".{ID}.ready"),            // no target name
            &format!(".inbox.org.{ID}.swp"),    // unknown phase
            ".inbox.org.short.ready", // malformed id
        ] {
            assert_eq!(parse_temp_name(name), None, "{name} should not parse");
            assert!(!is_legacy_temp(name), "{name} should not look legacy");
        }
    }

    #[test]
    fn an_interrupted_write_is_discarded() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("inbox.org"), "original").unwrap();
        temp(dir.path(), "inbox.org", PART, "half a n");

        let report = sweep(dir.path(), NOW);
        assert_eq!(report.discarded, 1);
        assert!(report.recovered.is_empty());
        // The half-written content must never reach the note.
        assert_eq!(
            std::fs::read_to_string(dir.path().join("inbox.org")).unwrap(),
            "original"
        );
    }

    #[test]
    fn a_complete_note_is_put_back_when_its_target_is_gone() {
        let dir = TempDir::new().unwrap();
        temp(dir.path(), "inbox.org", READY, "* TODO the edit");

        let report = sweep(dir.path(), NOW);
        assert_eq!(report.recovered.len(), 1);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("inbox.org")).unwrap(),
            "* TODO the edit"
        );
    }

    #[test]
    fn a_leftover_matching_its_target_is_just_tidied_away() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("inbox.org"), "same").unwrap();
        temp(dir.path(), "inbox.org", READY, "same");

        let report = sweep(dir.path(), NOW);
        assert_eq!(report.discarded, 1);
        assert!(report.recovered.is_empty());
        assert!(report.conflicted.is_empty());
    }

    #[test]
    fn a_complete_note_never_overwrites_a_different_target() {
        // The crash may have been days ago and the file pulled from git since.
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("inbox.org"), "pulled from git").unwrap();
        let ready = temp(dir.path(), "inbox.org", READY, "my interrupted edit");

        let report = sweep(dir.path(), NOW);
        assert_eq!(report.conflicted.len(), 1, "should refuse to choose");
        assert_eq!(report.discarded, 0);
        assert!(report.recovered.is_empty());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("inbox.org")).unwrap(),
            "pulled from git",
            "clobbered the newer file"
        );
        assert!(ready.exists(), "threw away the interrupted edit");
    }

    #[test]
    fn legacy_single_phase_temps_are_discarded() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(format!(".inbox.org.{ID}.tmp")), "x").unwrap();
        let report = sweep(dir.path(), NOW);
        assert_eq!(report.discarded, 1);
    }

    #[test]
    fn a_write_in_flight_is_left_alone() {
        let dir = TempDir::new().unwrap();
        temp(dir.path(), "inbox.org", READY, "being written right now");
        let report = sweep(dir.path(), Duration::from_secs(3600));
        assert!(report.is_empty(), "disturbed a write in progress");
    }

    #[test]
    fn nested_directories_are_swept_but_git_is_not() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("daily")).unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        temp(&dir.path().join("daily"), "2026-08-21.org", READY, "note");
        temp(&dir.path().join(".git"), "x.org", PART, "junk");

        let report = sweep(dir.path(), NOW);
        assert_eq!(report.recovered.len(), 1);
        assert_eq!(report.discarded, 0, "reached into .git");
        assert!(dir.path().join(".git").join(format!(".x.org.{ID}.part")).exists());
    }

    #[test]
    fn recovery_restores_exactly_what_the_write_had() {
        // End to end: interrupt a real write between the two renames.
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("note.org");
        std::fs::write(&target, "old").unwrap();

        let id = "abcdefabcdefabcdefabcdefabcdefab";
        let ready = crate::atomic::temp_path(&target, id, READY);
        std::fs::write(&ready, "* TODO brand new content").unwrap();
        std::fs::remove_file(&target).unwrap(); // publish deleted, then died

        let report = sweep(dir.path(), NOW);
        assert_eq!(report.recovered.len(), 1);
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "* TODO brand new content"
        );
    }
}

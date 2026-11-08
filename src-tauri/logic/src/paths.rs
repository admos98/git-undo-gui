//! Path-overlap rules for the two strategies that write the working tree.
//!
//! `git reset --hard` and `git restore --source` both write every path the
//! snapshot tracks. They do it without asking, even when a file already exists
//! there — and even when the snapshot wants a directory where an untracked file
//! sits (or the reverse), in which case git deletes the existing entry outright.
//! `backup-before-recovery` holds commits, not untracked files, so none of that
//! work can be recovered afterwards.
//!
//! These helpers answer "which paths on disk would git touch?" without invoking
//! git, so the guard can refuse before anything runs.

use crate::state::DirtyFile;

/// Normalise whitespace and the trailing slash `git status --porcelain` uses for
/// a wholly untracked directory (`?? pkg/`).
fn normalize(path: &str) -> &str {
    path.trim().trim_end_matches('/')
}

/// Would writing `tree_path` touch the thing on disk at `disk_path`?
///
/// True for the same path, for a path inside a directory (in either direction),
/// and for the file/directory case, where git replaces the existing entry to
/// make room — deleting whatever was there.
pub fn collides(disk_path: &str, tree_path: &str) -> bool {
    let disk = normalize(disk_path);
    let tree = normalize(tree_path);
    if disk.is_empty() || tree.is_empty() {
        return false;
    }

    let disk_contains_tree = tree
        .strip_prefix(disk)
        .is_some_and(|rest| rest.starts_with('/'));
    let tree_contains_disk = disk
        .strip_prefix(tree)
        .is_some_and(|rest| rest.starts_with('/'));

    disk == tree || disk_contains_tree || tree_contains_disk
}

/// Does this on-disk path overlap anything the snapshot tracks?
pub fn overlaps_tree(path: &str, tree_paths: &[String]) -> bool {
    tree_paths.iter().any(|tree| collides(path, tree))
}

/// Dirty entries whose on-disk content a write of `tree_paths` would replace.
pub fn overlapping<'a>(dirty: &'a [DirtyFile], tree_paths: &[String]) -> Vec<&'a DirtyFile> {
    dirty.iter().filter(|file| overlaps_tree(&file.path, tree_paths)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DirtyFile;

    fn tree(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|p| (*p).to_string()).collect()
    }

    #[test]
    fn identical_paths_collide() {
        assert!(collides("src/a.ts", "src/a.ts"));
        assert!(collides("./src/a.ts", "./src/a.ts"));
    }

    #[test]
    fn an_untracked_file_where_the_snapshot_has_a_directory_collides() {
        assert!(collides("docs", "docs/guide.md"));
        // git status spells a wholly untracked directory with a trailing slash.
        assert!(collides("docs/", "docs/guide.md"));
    }

    #[test]
    fn an_untracked_directory_where_the_snapshot_has_a_file_collides() {
        assert!(collides("notes.txt/", "notes.txt"));
        assert!(collides("notes.txt/inner.txt", "notes.txt"));
    }

    #[test]
    fn a_path_inside_an_untracked_directory_collides() {
        assert!(collides("pkg/", "pkg/keep.txt"));
        assert!(collides("pkg/keep.txt", "pkg/keep.txt"));
    }

    #[test]
    fn names_that_merely_share_a_prefix_do_not_collide() {
        for (disk, tree_path) in [
            ("src/a.ts", "src/b.ts"),
            ("src", "src2/a.ts"),
            ("src/a.tsx", "src/a.ts"),
            ("docs/a.md", "docs2"),
            ("README.md", "README"),
        ] {
            assert!(!collides(disk, tree_path), "{disk:?} collided with {tree_path:?}");
        }
    }

    #[test]
    fn empty_paths_never_collide() {
        assert!(!collides("", "src/a.ts"));
        assert!(!collides("src/a.ts", ""));
        assert!(!collides("/", "src/a.ts"));
    }

    #[test]
    fn overlapping_reports_exactly_the_touched_entries() {
        let dirty = vec![
            DirtyFile { status: "untracked".into(), path: "notes.md".into() },
            DirtyFile { status: "untracked".into(), path: "safe.txt".into() },
            DirtyFile { status: "modified".into(), path: "src/a.ts".into() },
        ];
        let hits = overlapping(&dirty, &tree(&["notes.md", "src/a.ts", "docs/x.md"]));
        let paths: Vec<&str> = hits.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["notes.md", "src/a.ts"]);
    }

    #[test]
    fn a_clean_tree_overlaps_nothing() {
        assert!(!overlaps_tree("anything", &[]));
    }
}

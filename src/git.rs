use crate::analyzer::analyze_file_diff;
use crate::models::EditReport;
use git2::{Delta, DiffOptions, Repository};
use std::error::Error;
use std::path::{Path, PathBuf};

pub fn open_repository(path: Option<&Path>) -> Result<Repository, Box<dyn Error>> {
    let repo = match path {
        Some(p) => Repository::discover(p)?,
        None => Repository::discover(".")?,
    };
    Ok(repo)
}

pub fn diff_repository(
    repo: &Repository,
    base_ref: &str,
    target_ref: Option<&str>,
) -> Result<EditReport, Box<dyn Error>> {
    let base_obj = repo.revparse_single(base_ref)?;
    let base_commit = base_obj.peel_to_commit()?;
    let base_tree = base_commit.tree()?;

    let mut opts = DiffOptions::new();
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);

    let diff = match target_ref {
        Some(target) => {
            let target_obj = repo.revparse_single(target)?;
            let target_commit = target_obj.peel_to_commit()?;
            let target_tree = target_commit.tree()?;
            repo.diff_tree_to_tree(Some(&base_tree), Some(&target_tree), Some(&mut opts))?
        }
        None => repo.diff_tree_to_workdir_with_index(Some(&base_tree), Some(&mut opts))?,
    };

    let mut report = EditReport::default();
    let num_deltas = diff.deltas().len();

    for i in 0..num_deltas {
        let delta = diff.get_delta(i).ok_or("Failed to get diff delta")?;
        let old_file = delta.old_file();
        let new_file = delta.new_file();
        let status = delta.status();

        let file_path = new_file
            .path()
            .or_else(|| old_file.path())
            .unwrap_or_else(|| Path::new("unknown"));

        let old_content = match status {
            Delta::Added | Delta::Untracked => None,
            _ => {
                if old_file.id().is_zero() {
                    None
                } else if let Ok(blob) = repo.find_blob(old_file.id()) {
                    std::str::from_utf8(blob.content())
                        .ok()
                        .map(|s| s.to_string())
                } else {
                    None
                }
            }
        };

        let new_content = match status {
            Delta::Deleted => None,
            _ => {
                if target_ref.is_some() {
                    if new_file.id().is_zero() {
                        None
                    } else if let Ok(blob) = repo.find_blob(new_file.id()) {
                        std::str::from_utf8(blob.content())
                            .ok()
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                } else {
                    // Diff against working tree on disk
                    if let Some(rel_path) = new_file.path() {
                        let full_path = repo
                            .workdir()
                            .map(|w| w.join(rel_path))
                            .unwrap_or_else(|| rel_path.to_path_buf());
                        std::fs::read_to_string(&full_path).ok()
                    } else {
                        None
                    }
                }
            }
        };

        let events = analyze_file_diff(file_path, old_content.as_deref(), new_content.as_deref());

        for event in events {
            report.add_event(event);
        }
    }

    Ok(report)
}

pub fn analyze_paths(paths: &[PathBuf]) -> Result<EditReport, Box<dyn Error>> {
    let mut report = EditReport::default();
    for p in paths {
        if p.is_dir() {
            for entry in walkdir::WalkDir::new(p).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file()
                    && entry.path().extension().and_then(|ext| ext.to_str()) == Some("rs")
                {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        let events = analyze_file_diff(entry.path(), None, Some(&content));
                        for event in events {
                            report.add_event(event);
                        }
                    }
                }
            }
        } else if p.is_file() {
            if let Ok(content) = std::fs::read_to_string(p) {
                let events = analyze_file_diff(p, None, Some(&content));
                for event in events {
                    report.add_event(event);
                }
            }
        }
    }
    Ok(report)
}

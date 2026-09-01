use crate::analyzer::analyze_file_diff;
use crate::models::EditReport;
use git2::{Delta, DiffFindOptions, DiffOptions, Repository};
use std::error::Error;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffConfig {
    pub find_renames: bool,
    pub rename_threshold: Option<u32>,
    pub ignore_patterns: Vec<String>,
}

fn match_glob(pattern: &str, text: &str) -> bool {
    let p_bytes = pattern.as_bytes();
    let t_bytes = text.as_bytes();
    let mut p_idx = 0;
    let mut t_idx = 0;
    let mut star_p_idx = None;
    let mut star_t_idx = 0;

    while t_idx < t_bytes.len() {
        if p_idx < p_bytes.len() && (p_bytes[p_idx] == b'?' || p_bytes[p_idx] == t_bytes[t_idx]) {
            p_idx += 1;
            t_idx += 1;
        } else if p_idx < p_bytes.len() && p_bytes[p_idx] == b'*' {
            star_p_idx = Some(p_idx);
            p_idx += 1;
            star_t_idx = t_idx;
        } else if let Some(sp) = star_p_idx {
            p_idx = sp + 1;
            star_t_idx += 1;
            t_idx = star_t_idx;
        } else {
            return false;
        }
    }

    while p_idx < p_bytes.len() && p_bytes[p_idx] == b'*' {
        p_idx += 1;
    }

    p_idx == p_bytes.len()
}

impl DiffConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_renames(mut self, find_renames: bool) -> Self {
        self.find_renames = find_renames;
        self
    }

    pub fn with_threshold(mut self, threshold: Option<u32>) -> Self {
        self.rename_threshold = threshold;
        self
    }

    pub fn with_ignore_patterns(mut self, patterns: Vec<String>) -> Self {
        self.ignore_patterns = patterns;
        self
    }

    pub fn should_ignore(&self, path: &Path) -> bool {
        if self.ignore_patterns.is_empty() {
            return false;
        }

        let path_str = path.to_string_lossy().replace('\\', "/");
        let path_str = path_str.trim_start_matches("./");
        let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");

        for raw_pattern in &self.ignore_patterns {
            let pattern = raw_pattern.replace('\\', "/");
            let pattern = pattern.trim_start_matches("./");

            if pattern.is_empty() {
                continue;
            }

            // Direct match or wildcard match on full path or filename
            if path_str == pattern || file_name == pattern {
                return true;
            }

            if match_glob(pattern, path_str) || match_glob(pattern, file_name) {
                return true;
            }

            // Directory component or prefix match
            let clean_dir = pattern.trim_end_matches('/');
            if path_str.starts_with(&format!("{}/", clean_dir))
                || path_str.contains(&format!("/{}/", clean_dir))
                || path_str == clean_dir
            {
                return true;
            }
        }

        false
    }
}

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
    config: Option<&DiffConfig>,
) -> Result<EditReport, Box<dyn Error>> {
    let base_obj = repo.revparse_single(base_ref)?;
    let base_commit = base_obj.peel_to_commit()?;
    let base_tree = base_commit.tree()?;

    let mut opts = DiffOptions::new();
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);

    let mut diff = match target_ref {
        Some(target) => {
            let target_obj = repo.revparse_single(target)?;
            let target_commit = target_obj.peel_to_commit()?;
            let target_tree = target_commit.tree()?;
            repo.diff_tree_to_tree(Some(&base_tree), Some(&target_tree), Some(&mut opts))?
        }
        None => repo.diff_tree_to_workdir_with_index(Some(&base_tree), Some(&mut opts))?,
    };

    if let Some(cfg) = config {
        if cfg.find_renames || cfg.rename_threshold.is_some() {
            let mut find_opts = DiffFindOptions::new();
            find_opts.renames(true);
            find_opts.renames_from_rewrites(true);
            if let Some(threshold) = cfg.rename_threshold {
                find_opts.rename_threshold(threshold as u16);
            }
            diff.find_similar(Some(&mut find_opts))?;
        }
    }

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

        if let Some(cfg) = config {
            if cfg.should_ignore(file_path) {
                continue;
            }
            if let Some(old_p) = old_file.path() {
                if cfg.should_ignore(old_p) {
                    continue;
                }
            }
        }

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

fn is_supported_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "rs" | "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs" | "py" | "cs"
            )
        })
        .unwrap_or(false)
}

pub fn analyze_paths(
    paths: &[PathBuf],
    config: Option<&DiffConfig>,
) -> Result<EditReport, Box<dyn Error>> {
    let mut report = EditReport::default();
    for p in paths {
        if let Some(cfg) = config {
            if cfg.should_ignore(p) {
                continue;
            }
        }
        if p.is_dir() {
            for entry in walkdir::WalkDir::new(p).into_iter().filter_map(|e| e.ok()) {
                if let Some(cfg) = config {
                    if cfg.should_ignore(entry.path()) {
                        continue;
                    }
                }
                if entry.file_type().is_file() && is_supported_file(entry.path()) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_diff_config_defaults_and_builder() {
        let config = DiffConfig::new()
            .with_renames(true)
            .with_threshold(Some(80))
            .with_ignore_patterns(vec!["vendor/*".to_string(), "*.min.js".to_string()]);

        assert!(config.find_renames);
        assert_eq!(config.rename_threshold, Some(80));
        assert_eq!(config.ignore_patterns.len(), 2);
    }

    #[test]
    fn test_diff_config_should_ignore_prefix_and_dir() {
        let config = DiffConfig {
            find_renames: false,
            rename_threshold: None,
            ignore_patterns: vec!["vendor/".to_string(), "target".to_string()],
        };

        assert!(config.should_ignore(Path::new("vendor/lib.rs")));
        assert!(config.should_ignore(Path::new("src/vendor/lib.rs")));
        assert!(config.should_ignore(Path::new("target/debug/build.rs")));
        assert!(!config.should_ignore(Path::new("src/main.rs")));
    }

    #[test]
    fn test_diff_config_should_ignore_wildcards() {
        let config = DiffConfig {
            find_renames: false,
            rename_threshold: None,
            ignore_patterns: vec!["*.min.js".to_string(), "*.generated.rs".to_string()],
        };

        assert!(config.should_ignore(Path::new("static/bundle.min.js")));
        assert!(config.should_ignore(Path::new("src/proto.generated.rs")));
        assert!(!config.should_ignore(Path::new("src/proto.rs")));
    }

    #[test]
    fn test_diff_config_empty_does_not_ignore() {
        let config = DiffConfig::default();
        assert!(!config.should_ignore(Path::new("src/main.rs")));
    }

    #[test]
    fn test_analyze_paths_with_ignore_patterns() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        let src_dir = root.join("src");
        let vendor_dir = root.join("vendor");
        let target_dir = root.join("target");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&vendor_dir).unwrap();
        std::fs::create_dir_all(&target_dir).unwrap();

        let main_file = src_dir.join("main.rs");
        let vendor_file = vendor_dir.join("dep.rs");
        let min_file = target_dir.join("bundle.min.js");

        std::fs::write(&main_file, "fn main() {}\n").unwrap();
        std::fs::write(&vendor_file, "pub fn vendor_fn() {}\n").unwrap();
        std::fs::write(&min_file, "function min() {}\n").unwrap();

        let config = DiffConfig {
            find_renames: false,
            rename_threshold: None,
            ignore_patterns: vec!["vendor/".to_string(), "*.min.js".to_string()],
        };

        let report = analyze_paths(&[root.to_path_buf()], Some(&config)).unwrap();
        assert_eq!(report.files_added, 1);
        assert_eq!(report.functions_added, 1);
        assert_eq!(report.total_edits, 2);
        assert!(report
            .events
            .iter()
            .any(|e| e.symbol.as_deref() == Some("main")));
        assert!(!report
            .events
            .iter()
            .any(|e| e.symbol.as_deref() == Some("vendor_fn")));
    }

    #[test]
    fn test_analyze_paths_none_config() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        let src_dir = root.join("src");
        let vendor_dir = root.join("vendor");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&vendor_dir).unwrap();

        let main_file = src_dir.join("main.rs");
        let vendor_file = vendor_dir.join("dep.rs");

        std::fs::write(&main_file, "fn main() {}\n").unwrap();
        std::fs::write(&vendor_file, "pub fn vendor_fn() {}\n").unwrap();

        let report = analyze_paths(&[root.to_path_buf()], None).unwrap();
        assert_eq!(report.files_added, 2);
        assert_eq!(report.functions_added, 2);
        assert_eq!(report.total_edits, 4);
    }
}

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "edit-counter",
    version,
    about = "Count and track semantic code edits (classes, functions, files, modifications, deletions) for AI agents."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to file or directory to inspect (defaults to current directory)
    #[arg(short, long, global = true)]
    path: Option<PathBuf>,

    /// Output results as JSON
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Count semantic edits in a git diff or working tree
    Diff {
        /// Base git ref or commit to compare against
        #[arg(default_value = "HEAD~1")]
        base: String,
        /// Target git ref (defaults to working tree)
        target: Option<String>,
    },
    /// Analyze files and count code units (classes, functions, structs)
    Analyze {
        /// Paths to files or directories to analyze
        paths: Vec<PathBuf>,
    },
    /// Show summary edit metrics
    Summary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EditKind {
    FileAdded,
    FileModified,
    FileDeleted,
    ClassAdded,
    ClassModified,
    ClassDeleted,
    FunctionAdded,
    FunctionModified,
    FunctionDeleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditEvent {
    pub kind: EditKind,
    pub symbol: Option<String>,
    pub file: PathBuf,
    pub line: Option<usize>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EditReport {
    pub total_edits: usize,
    pub files_added: usize,
    pub files_modified: usize,
    pub files_deleted: usize,
    pub classes_added: usize,
    pub classes_modified: usize,
    pub classes_deleted: usize,
    pub functions_added: usize,
    pub functions_modified: usize,
    pub functions_deleted: usize,
    pub events: Vec<EditEvent>,
}

impl EditReport {
    pub fn add_event(&mut self, event: EditEvent) {
        match event.kind {
            EditKind::FileAdded => self.files_added += 1,
            EditKind::FileModified => self.files_modified += 1,
            EditKind::FileDeleted => self.files_deleted += 1,
            EditKind::ClassAdded => self.classes_added += 1,
            EditKind::ClassModified => self.classes_modified += 1,
            EditKind::ClassDeleted => self.classes_deleted += 1,
            EditKind::FunctionAdded => self.functions_added += 1,
            EditKind::FunctionModified => self.functions_modified += 1,
            EditKind::FunctionDeleted => self.functions_deleted += 1,
        }
        self.total_edits += 1;
        self.events.push(event);
    }
}

fn main() {
    let cli = Cli::parse();

    let report = EditReport::default();

    match cli.command {
        Some(Commands::Diff { base, target }) => {
            let target_str = target.as_deref().unwrap_or("working tree");
            eprintln!("Analyzing edits between {} and {}...", base, target_str);
        }
        Some(Commands::Analyze { paths }) => {
            eprintln!("Analyzing {} path(s)...", paths.len());
        }
        Some(Commands::Summary) | None => {
            eprintln!("Edit Counter v{}", env!("CARGO_PKG_VERSION"));
        }
    }

    if cli.json {
        let json = serde_json::to_string_pretty(&report).unwrap();
        println!("{}", json);
    } else {
        println!("Total edits: {}", report.total_edits);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_report() {
        let report = EditReport::default();
        assert_eq!(report.total_edits, 0);
    }

    #[test]
    fn test_add_events() {
        let mut report = EditReport::default();
        report.add_event(EditEvent {
            kind: EditKind::FunctionAdded,
            symbol: Some("calculate_edits".to_string()),
            file: PathBuf::from("src/main.rs"),
            line: Some(42),
        });
        report.add_event(EditKind::ClassAdded.into_event("EditReport", "src/main.rs", 10));

        assert_eq!(report.total_edits, 2);
        assert_eq!(report.functions_added, 1);
        assert_eq!(report.classes_added, 1);
    }

    impl EditKind {
        fn into_event(self, symbol: &str, file: &str, line: usize) -> EditEvent {
            EditEvent {
                kind: self,
                symbol: Some(symbol.to_string()),
                file: PathBuf::from(file),
                line: Some(line),
            }
        }
    }
}

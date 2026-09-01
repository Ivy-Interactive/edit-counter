use clap::{Parser, Subcommand};
use edit_counter::{analyze_paths, diff_repository, open_repository, DiffConfig, EditReport};
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

    /// Enable rename detection in git diffs
    #[arg(long, global = true)]
    find_renames: bool,

    /// Similarity threshold for rename detection (0-100)
    #[arg(long, global = true)]
    rename_threshold: Option<u32>,

    /// Ignore patterns for files or directories (e.g. vendor/, *.min.js)
    #[arg(short, long = "ignore", global = true)]
    ignore: Vec<String>,

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

fn main() {
    let cli = Cli::parse();

    let diff_config = DiffConfig {
        find_renames: cli.find_renames,
        rename_threshold: cli.rename_threshold,
        ignore_patterns: cli.ignore.clone(),
    };

    let report = match &cli.command {
        Some(Commands::Diff { base, target }) => match open_repository(cli.path.as_deref()) {
            Ok(repo) => match diff_repository(&repo, base, target.as_deref(), Some(&diff_config)) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Error computing diff: {}", e);
                    EditReport::default()
                }
            },
            Err(e) => {
                eprintln!("Error opening repository: {}", e);
                EditReport::default()
            }
        },
        Some(Commands::Analyze { paths }) => {
            let target_paths = if paths.is_empty() {
                vec![cli.path.clone().unwrap_or_else(|| PathBuf::from("."))]
            } else {
                paths.clone()
            };
            match analyze_paths(&target_paths, Some(&diff_config)) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Error analyzing paths: {}", e);
                    EditReport::default()
                }
            }
        }
        Some(Commands::Summary) | None => match open_repository(cli.path.as_deref()) {
            Ok(repo) => diff_repository(&repo, "HEAD~1", None, Some(&diff_config))
                .or_else(|_| diff_repository(&repo, "HEAD", None, Some(&diff_config)))
                .unwrap_or_default(),
            Err(_) => {
                eprintln!("Edit Counter v{}", env!("CARGO_PKG_VERSION"));
                EditReport::default()
            }
        },
    };

    if cli.json {
        let json = serde_json::to_string_pretty(&report).unwrap();
        println!("{}", json);
    } else {
        println!("Total edits: {}", report.total_edits);
    }
}

#[cfg(test)]
mod tests {
    use edit_counter::{EditEvent, EditKind, EditReport};
    use std::path::PathBuf;

    #[test]
    fn test_empty_report() {
        let report = EditReport::default();
        assert_eq!(report.total_edits, 0);
        assert_eq!(report.files_added, 0);
        assert_eq!(report.files_modified, 0);
        assert_eq!(report.files_deleted, 0);
        assert_eq!(report.classes_added, 0);
        assert_eq!(report.classes_modified, 0);
        assert_eq!(report.classes_deleted, 0);
        assert_eq!(report.functions_added, 0);
        assert_eq!(report.functions_modified, 0);
        assert_eq!(report.functions_deleted, 0);
        assert!(report.events.is_empty());
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
        report.add_event(EditKind::ClassAdded.into_event(
            Some("EditReport"),
            "src/main.rs",
            Some(10),
        ));

        assert_eq!(report.total_edits, 2);
        assert_eq!(report.functions_added, 1);
        assert_eq!(report.classes_added, 1);
    }

    /// Verifies file lifecycle events: adding, modifying, and deleting files.
    /// Expected breakdown: 1 FileAdded + 1 FileModified + 1 FileDeleted = 3 edits.
    #[test]
    fn test_file_lifecycle_edits() {
        let mut report = EditReport::default();

        report.add_event(EditKind::FileAdded.into_event(None, "src/lib.rs", None));
        report.add_event(EditKind::FileModified.into_event(None, "src/lib.rs", None));
        report.add_event(EditKind::FileDeleted.into_event(None, "src/old.rs", None));

        assert_eq!(report.total_edits, 3);
        assert_eq!(report.files_added, 1);
        assert_eq!(report.files_modified, 1);
        assert_eq!(report.files_deleted, 1);
        assert_eq!(report.events.len(), 3);
    }

    /// Verifies class and struct lifecycle events: adding, modifying, and deleting types.
    /// Expected breakdown: 1 ClassAdded + 1 ClassModified + 1 ClassDeleted = 3 edits.
    #[test]
    fn test_class_lifecycle_edits() {
        let mut report = EditReport::default();

        report.add_event(EditKind::ClassAdded.into_event(
            Some("UserSession"),
            "src/auth.rs",
            Some(5),
        ));
        report.add_event(EditKind::ClassModified.into_event(
            Some("UserSession"),
            "src/auth.rs",
            Some(5),
        ));
        report.add_event(EditKind::ClassDeleted.into_event(
            Some("OldSession"),
            "src/auth.rs",
            Some(50),
        ));

        assert_eq!(report.total_edits, 3);
        assert_eq!(report.classes_added, 1);
        assert_eq!(report.classes_modified, 1);
        assert_eq!(report.classes_deleted, 1);
        assert_eq!(report.events.len(), 3);
    }

    /// Verifies function and method lifecycle events: adding, modifying, and deleting functions.
    /// Expected breakdown: 1 FunctionAdded + 1 FunctionModified + 1 FunctionDeleted = 3 edits.
    #[test]
    fn test_function_lifecycle_edits() {
        let mut report = EditReport::default();

        report.add_event(EditKind::FunctionAdded.into_event(
            Some("authenticate"),
            "src/auth.rs",
            Some(12),
        ));
        report.add_event(EditKind::FunctionModified.into_event(
            Some("authenticate"),
            "src/auth.rs",
            Some(12),
        ));
        report.add_event(EditKind::FunctionDeleted.into_event(
            Some("legacy_login"),
            "src/auth.rs",
            Some(40),
        ));

        assert_eq!(report.total_edits, 3);
        assert_eq!(report.functions_added, 1);
        assert_eq!(report.functions_modified, 1);
        assert_eq!(report.functions_deleted, 1);
        assert_eq!(report.events.len(), 3);
    }

    /// Tests a composite scenario where a developer introduces a new module with a struct and two methods.
    /// Scenario: 1 file added (src/auth.rs), 1 struct added (UserAuth), 2 methods added (login, logout).
    /// Expected breakdown: 1 FileAdded + 1 ClassAdded + 2 FunctionAdded = 4 edits.
    #[test]
    fn test_composite_scenario_new_feature() {
        let mut report = EditReport::default();

        report.add_event(EditKind::FileAdded.into_event(None, "src/auth.rs", None));
        report.add_event(EditKind::ClassAdded.into_event(Some("UserAuth"), "src/auth.rs", Some(1)));
        report.add_event(EditKind::FunctionAdded.into_event(
            Some("login"),
            "src/auth.rs",
            Some(10),
        ));
        report.add_event(EditKind::FunctionAdded.into_event(
            Some("logout"),
            "src/auth.rs",
            Some(25),
        ));

        assert_eq!(report.total_edits, 4);
        assert_eq!(report.files_added, 1);
        assert_eq!(report.classes_added, 1);
        assert_eq!(report.functions_added, 2);
        assert_eq!(report.events.len(), 4);
    }

    /// Tests a composite scenario where a developer refactors two existing functions inside an existing file.
    /// Scenario: 1 file modified (src/billing.rs), 2 functions modified (apply_discount, compute_tax).
    /// Expected breakdown: 1 FileModified + 2 FunctionModified = 3 edits.
    #[test]
    fn test_composite_scenario_function_refactor() {
        let mut report = EditReport::default();

        report.add_event(EditKind::FileModified.into_event(None, "src/billing.rs", None));
        report.add_event(EditKind::FunctionModified.into_event(
            Some("apply_discount"),
            "src/billing.rs",
            Some(30),
        ));
        report.add_event(EditKind::FunctionModified.into_event(
            Some("compute_tax"),
            "src/billing.rs",
            Some(60),
        ));

        assert_eq!(report.total_edits, 3);
        assert_eq!(report.files_modified, 1);
        assert_eq!(report.functions_modified, 2);
        assert_eq!(report.events.len(), 3);
    }

    /// Tests a composite scenario where an entire file containing 1 struct and 2 functions is deleted.
    /// Scenario: 1 file deleted (src/old_parser.rs), 1 struct deleted (OldParser), 2 functions deleted (parse, tokenize).
    /// Expected breakdown: 1 FileDeleted + 1 ClassDeleted + 2 FunctionDeleted = 4 edits.
    #[test]
    fn test_composite_scenario_file_deletion() {
        let mut report = EditReport::default();

        report.add_event(EditKind::FileDeleted.into_event(None, "src/old_parser.rs", None));
        report.add_event(EditKind::ClassDeleted.into_event(
            Some("OldParser"),
            "src/old_parser.rs",
            Some(1),
        ));
        report.add_event(EditKind::FunctionDeleted.into_event(
            Some("parse"),
            "src/old_parser.rs",
            Some(15),
        ));
        report.add_event(EditKind::FunctionDeleted.into_event(
            Some("tokenize"),
            "src/old_parser.rs",
            Some(45),
        ));

        assert_eq!(report.total_edits, 4);
        assert_eq!(report.files_deleted, 1);
        assert_eq!(report.classes_deleted, 1);
        assert_eq!(report.functions_deleted, 2);
        assert_eq!(report.events.len(), 4);
    }

    /// Tests JSON serialization and deserialization of EditReport and EditEvent.
    /// Verifies all fields roundtrip accurately and match specification expectations.
    #[test]
    fn test_json_serialization_matches_spec() {
        let mut report = EditReport::default();
        report.add_event(EditKind::FileAdded.into_event(None, "src/auth.rs", None));
        report.add_event(EditKind::ClassAdded.into_event(Some("UserAuth"), "src/auth.rs", Some(1)));
        report.add_event(EditKind::FunctionAdded.into_event(
            Some("login"),
            "src/auth.rs",
            Some(10),
        ));

        let json_str = serde_json::to_string(&report).expect("Serialization failed");
        let deserialized: EditReport =
            serde_json::from_str(&json_str).expect("Deserialization failed");

        assert_eq!(report, deserialized);
        assert_eq!(deserialized.total_edits, 3);
        assert_eq!(deserialized.files_added, 1);
        assert_eq!(deserialized.classes_added, 1);
        assert_eq!(deserialized.functions_added, 1);
        assert_eq!(deserialized.events.len(), 3);
        assert_eq!(deserialized.events[0].kind, EditKind::FileAdded);
        assert_eq!(deserialized.events[1].symbol.as_deref(), Some("UserAuth"));
        assert_eq!(deserialized.events[2].line, Some(10));
    }
}

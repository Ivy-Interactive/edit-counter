use edit_counter::{diff_repository, EditKind};
use git2::{Commit, Oid, Repository, Signature};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn init_test_repo() -> (TempDir, Repository) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo = Repository::init(temp_dir.path()).expect("Failed to init git repo");

    let mut config = repo.config().expect("Failed to get config");
    config
        .set_str("user.name", "Test User")
        .expect("Failed to set user.name");
    config
        .set_str("user.email", "test@example.com")
        .expect("Failed to set user.email");

    (temp_dir, repo)
}

fn commit_file(
    repo: &Repository,
    rel_path: &str,
    content: Option<&str>,
    msg: &str,
    parents: &[&Commit],
) -> Oid {
    let workdir = repo.workdir().expect("Repo has no workdir");
    let full_path = workdir.join(rel_path);

    if let Some(c) = content {
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).expect("Failed to create parent dirs");
        }
        fs::write(&full_path, c).expect("Failed to write file");
        let mut index = repo.index().expect("Failed to get index");
        index
            .add_path(Path::new(rel_path))
            .expect("Failed to add path to index");
        index.write().expect("Failed to write index");
    } else {
        if full_path.exists() {
            fs::remove_file(&full_path).expect("Failed to remove file");
        }
        let mut index = repo.index().expect("Failed to get index");
        index
            .remove_path(Path::new(rel_path))
            .expect("Failed to remove path from index");
        index.write().expect("Failed to write index");
    }

    let mut index = repo.index().expect("Failed to get index");
    let tree_id = index.write_tree().expect("Failed to write tree");
    let tree = repo.find_tree(tree_id).expect("Failed to find tree");

    let sig = Signature::now("Test User", "test@example.com").expect("Failed to create signature");
    repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, parents)
        .expect("Failed to commit")
}

#[test]
fn test_git_diff_example_1_new_feature_file() {
    let (_tmp, repo) = init_test_repo();

    let c1_oid = commit_file(
        &repo,
        "README.md",
        Some("# Test Repo"),
        "Initial commit",
        &[],
    );
    let c1 = repo.find_commit(c1_oid).expect("Find c1");

    let auth_code = r#"
pub struct UserAuth {
    pub username: String,
}

impl UserAuth {
    pub fn login(&self) -> bool {
        true
    }

    pub fn logout(&self) {}
}
"#;
    let c2_oid = commit_file(
        &repo,
        "src/auth.rs",
        Some(auth_code),
        "Add auth module",
        &[&c1],
    );

    let report = diff_repository(&repo, &c1_oid.to_string(), Some(&c2_oid.to_string()))
        .expect("diff_repository failed");

    assert_eq!(report.total_edits, 4);
    assert_eq!(report.files_added, 1);
    assert_eq!(report.classes_added, 1);
    assert_eq!(report.functions_added, 2);
    assert_eq!(report.files_modified, 0);
    assert_eq!(report.files_deleted, 0);

    let symbols: Vec<Option<String>> = report.events.iter().map(|e| e.symbol.clone()).collect();
    assert!(symbols.contains(&None)); // FileAdded
    assert!(symbols.contains(&Some("UserAuth".to_string())));
    assert!(symbols.contains(&Some("login".to_string())));
    assert!(symbols.contains(&Some("logout".to_string())));
}

#[test]
fn test_git_diff_example_2_refactor_function() {
    let (_tmp, repo) = init_test_repo();

    let initial_billing = r#"
pub fn calculate_tax(amount: f64) -> f64 {
    amount * 0.1
}
"#;
    let c1_oid = commit_file(
        &repo,
        "src/billing.rs",
        Some(initial_billing),
        "Initial billing",
        &[],
    );
    let c1 = repo.find_commit(c1_oid).expect("Find c1");

    let updated_billing = r#"
pub fn calculate_tax(amount: f64) -> f64 {
    amount * 0.15
}
"#;
    let c2_oid = commit_file(
        &repo,
        "src/billing.rs",
        Some(updated_billing),
        "Update tax rate",
        &[&c1],
    );

    let report = diff_repository(&repo, &c1_oid.to_string(), Some(&c2_oid.to_string()))
        .expect("diff_repository failed");

    assert_eq!(report.total_edits, 2);
    assert_eq!(report.files_modified, 1);
    assert_eq!(report.functions_modified, 1);
    assert_eq!(report.files_added, 0);
    assert_eq!(report.files_deleted, 0);
    assert_eq!(report.events[0].kind, EditKind::FileModified);
    assert_eq!(report.events[1].kind, EditKind::FunctionModified);
    assert_eq!(report.events[1].symbol.as_deref(), Some("calculate_tax"));
}

#[test]
fn test_git_diff_example_3_delete_class_and_methods() {
    let (_tmp, repo) = init_test_repo();

    let legacy_code = r#"
pub struct LegacySession {
    pub token: String,
}

impl LegacySession {
    pub fn init() {}
    pub fn validate() {}
    pub fn close() {}
}
"#;
    let c1_oid = commit_file(
        &repo,
        "src/legacy.rs",
        Some(legacy_code),
        "Initial legacy",
        &[],
    );
    let c1 = repo.find_commit(c1_oid).expect("Find c1");

    let updated_code = r#"
// Legacy removed
"#;
    let c2_oid = commit_file(
        &repo,
        "src/legacy.rs",
        Some(updated_code),
        "Remove legacy session",
        &[&c1],
    );

    let report = diff_repository(&repo, &c1_oid.to_string(), Some(&c2_oid.to_string()))
        .expect("diff_repository failed");

    assert_eq!(report.total_edits, 5);
    assert_eq!(report.files_modified, 1);
    assert_eq!(report.classes_deleted, 1);
    assert_eq!(report.functions_deleted, 3);
    assert_eq!(report.files_added, 0);
    assert_eq!(report.files_deleted, 0);

    let symbols: Vec<Option<String>> = report.events.iter().map(|e| e.symbol.clone()).collect();
    assert!(symbols.contains(&Some("LegacySession".to_string())));
    assert!(symbols.contains(&Some("init".to_string())));
    assert!(symbols.contains(&Some("validate".to_string())));
    assert!(symbols.contains(&Some("close".to_string())));
}

#[test]
fn test_git_diff_example_4_delete_entire_file() {
    let (_tmp, repo) = init_test_repo();

    let widget_code = r#"
pub struct OldWidget;

pub fn draw_widget() {}
"#;
    let c1_oid = commit_file(
        &repo,
        "src/old_widget.rs",
        Some(widget_code),
        "Initial widget",
        &[],
    );
    let c1 = repo.find_commit(c1_oid).expect("Find c1");

    let c2_oid = commit_file(
        &repo,
        "src/old_widget.rs",
        None,
        "Delete widget file",
        &[&c1],
    );

    let report = diff_repository(&repo, &c1_oid.to_string(), Some(&c2_oid.to_string()))
        .expect("diff_repository failed");

    assert_eq!(report.total_edits, 3);
    assert_eq!(report.files_deleted, 1);
    assert_eq!(report.classes_deleted, 1);
    assert_eq!(report.functions_deleted, 1);
    assert_eq!(report.files_added, 0);
    assert_eq!(report.files_modified, 0);
    assert_eq!(report.events[0].kind, EditKind::FileDeleted);
    assert_eq!(report.events[1].kind, EditKind::ClassDeleted);
    assert_eq!(report.events[1].symbol.as_deref(), Some("OldWidget"));
    assert_eq!(report.events[2].kind, EditKind::FunctionDeleted);
    assert_eq!(report.events[2].symbol.as_deref(), Some("draw_widget"));
}

#[test]
fn test_git_diff_typescript_feature() {
    let (_tmp, repo) = init_test_repo();

    let c1_oid = commit_file(
        &repo,
        "package.json",
        Some("{\"name\": \"my-app\"}"),
        "Initial commit",
        &[],
    );
    let c1 = repo.find_commit(c1_oid).expect("Find c1");

    let ts_code = r#"
export interface UserDTO {
    id: string;
}

export class UserService {
    fetchUser(id: string): UserDTO {
        return { id };
    }
}
"#;
    let c2_oid = commit_file(
        &repo,
        "src/user.ts",
        Some(ts_code),
        "Add user service",
        &[&c1],
    );

    let report = diff_repository(&repo, &c1_oid.to_string(), Some(&c2_oid.to_string()))
        .expect("diff_repository failed");

    assert_eq!(report.files_added, 1);
    assert_eq!(report.classes_added, 2); // UserDTO, UserService
    assert_eq!(report.functions_added, 1); // fetchUser
    assert_eq!(report.total_edits, 4);
}

#[test]
fn test_git_diff_python_refactor() {
    let (_tmp, repo) = init_test_repo();

    let py_code_v1 = r#"
class TaskRunner:
    def run(self):
        print("v1")
"#;
    let c1_oid = commit_file(&repo, "tasks.py", Some(py_code_v1), "Initial runner", &[]);
    let c1 = repo.find_commit(c1_oid).expect("Find c1");

    let py_code_v2 = r#"
class TaskRunner:
    def run(self):
        print("v2")

    def cancel(self):
        pass
"#;
    let c2_oid = commit_file(&repo, "tasks.py", Some(py_code_v2), "Update runner", &[&c1]);

    let report = diff_repository(&repo, &c1_oid.to_string(), Some(&c2_oid.to_string()))
        .expect("diff_repository failed");

    assert_eq!(report.files_modified, 1);
    assert_eq!(report.functions_modified, 1);
    assert_eq!(report.functions_added, 1);
}

#[test]
fn test_git_diff_csharp_feature() {
    let (_tmp, repo) = init_test_repo();

    let c1_oid = commit_file(&repo, "App.sln", Some(""), "Initial solution", &[]);
    let c1 = repo.find_commit(c1_oid).expect("Find c1");

    let cs_code = r#"
namespace MyApp
{
    public class OrderController
    {
        public OrderController() {}
        public void Process() {}
    }
}
"#;
    let c2_oid = commit_file(
        &repo,
        "Controllers/OrderController.cs",
        Some(cs_code),
        "Add OrderController",
        &[&c1],
    );

    let report = diff_repository(&repo, &c1_oid.to_string(), Some(&c2_oid.to_string()))
        .expect("diff_repository failed");

    assert_eq!(report.files_added, 1);
    assert_eq!(report.classes_added, 1);
    assert_eq!(report.functions_added, 2); // constructor + method
    assert_eq!(report.total_edits, 4);
}

#[test]
fn test_git_diff_javascript_feature() {
    let (_tmp, repo) = init_test_repo();

    let c1_oid = commit_file(&repo, "package.json", Some("{}"), "Initial", &[]);
    let c1 = repo.find_commit(c1_oid).expect("Find c1");

    let js_code = r#"
class AuthHelper {
    login() {}
}

const check = () => true;
"#;
    let c2_oid = commit_file(&repo, "auth.js", Some(js_code), "Add auth", &[&c1]);

    let report = diff_repository(&repo, &c1_oid.to_string(), Some(&c2_oid.to_string()))
        .expect("diff_repository failed");

    assert_eq!(report.files_added, 1);
    assert_eq!(report.classes_added, 1);
    assert_eq!(report.functions_added, 2);
    assert_eq!(report.total_edits, 4);
}

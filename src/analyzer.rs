use crate::ast::{parse_items_for_path, CodeItem, CodeItemKind};
use crate::models::{EditEvent, EditKind};
use std::path::Path;

pub fn compare_items(
    file_path: &Path,
    old_items: &[CodeItem],
    new_items: &[CodeItem],
) -> Vec<EditEvent> {
    let mut events = Vec::new();
    let mut matched_old = vec![false; old_items.len()];

    for new_item in new_items {
        // Match by (kind, name, parent) first, then (kind, name)
        let match_idx = old_items
            .iter()
            .enumerate()
            .position(|(i, old)| {
                !matched_old[i]
                    && old.kind == new_item.kind
                    && old.name == new_item.name
                    && old.parent == new_item.parent
            })
            .or_else(|| {
                old_items.iter().enumerate().position(|(i, old)| {
                    !matched_old[i] && old.kind == new_item.kind && old.name == new_item.name
                })
            });

        if let Some(old_idx) = match_idx {
            matched_old[old_idx] = true;
            let old_item = &old_items[old_idx];
            if old_item.tokens != new_item.tokens {
                let kind = match new_item.kind {
                    CodeItemKind::Class => EditKind::ClassModified,
                    CodeItemKind::Function => EditKind::FunctionModified,
                };
                events.push(EditEvent {
                    kind,
                    symbol: Some(new_item.name.clone()),
                    file: file_path.to_path_buf(),
                    line: new_item.line,
                });
            }
        } else {
            let kind = match new_item.kind {
                CodeItemKind::Class => EditKind::ClassAdded,
                CodeItemKind::Function => EditKind::FunctionAdded,
            };
            events.push(EditEvent {
                kind,
                symbol: Some(new_item.name.clone()),
                file: file_path.to_path_buf(),
                line: new_item.line,
            });
        }
    }

    for (old_idx, old_item) in old_items.iter().enumerate() {
        if !matched_old[old_idx] {
            let kind = match old_item.kind {
                CodeItemKind::Class => EditKind::ClassDeleted,
                CodeItemKind::Function => EditKind::FunctionDeleted,
            };
            events.push(EditEvent {
                kind,
                symbol: Some(old_item.name.clone()),
                file: file_path.to_path_buf(),
                line: old_item.line,
            });
        }
    }

    events
}

pub fn analyze_file_diff(
    file_path: &Path,
    old_content: Option<&str>,
    new_content: Option<&str>,
) -> Vec<EditEvent> {
    let mut events = Vec::new();

    match (old_content, new_content) {
        (None, Some(new_text)) => {
            // File Added
            events.push(EditEvent {
                kind: EditKind::FileAdded,
                symbol: None,
                file: file_path.to_path_buf(),
                line: None,
            });

            let items = parse_items_for_path(file_path, new_text);
            for item in items {
                let kind = match item.kind {
                    CodeItemKind::Class => EditKind::ClassAdded,
                    CodeItemKind::Function => EditKind::FunctionAdded,
                };
                events.push(EditEvent {
                    kind,
                    symbol: Some(item.name),
                    file: file_path.to_path_buf(),
                    line: item.line,
                });
            }
        }
        (Some(old_text), None) => {
            // File Deleted
            events.push(EditEvent {
                kind: EditKind::FileDeleted,
                symbol: None,
                file: file_path.to_path_buf(),
                line: None,
            });

            let items = parse_items_for_path(file_path, old_text);
            for item in items {
                let kind = match item.kind {
                    CodeItemKind::Class => EditKind::ClassDeleted,
                    CodeItemKind::Function => EditKind::FunctionDeleted,
                };
                events.push(EditEvent {
                    kind,
                    symbol: Some(item.name),
                    file: file_path.to_path_buf(),
                    line: item.line,
                });
            }
        }
        (Some(old_text), Some(new_text)) => {
            // Unchanged check
            if old_text == new_text {
                return events;
            }

            // File Modified
            events.push(EditEvent {
                kind: EditKind::FileModified,
                symbol: None,
                file: file_path.to_path_buf(),
                line: None,
            });

            let old_items = parse_items_for_path(file_path, old_text);
            let new_items = parse_items_for_path(file_path, new_text);

            let item_events = compare_items(file_path, &old_items, &new_items);
            events.extend(item_events);
        }
        (None, None) => {}
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_file_diff_add_file() {
        let code = r#"
pub struct UserAuth;

impl UserAuth {
    pub fn login() {}
    pub fn logout() {}
}
"#;
        let events = analyze_file_diff(Path::new("src/auth.rs"), None, Some(code));
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].kind, EditKind::FileAdded);
        assert_eq!(events[1].kind, EditKind::ClassAdded);
        assert_eq!(events[1].symbol.as_deref(), Some("UserAuth"));
        assert_eq!(events[2].kind, EditKind::FunctionAdded);
        assert_eq!(events[2].symbol.as_deref(), Some("login"));
        assert_eq!(events[3].kind, EditKind::FunctionAdded);
        assert_eq!(events[3].symbol.as_deref(), Some("logout"));
    }

    #[test]
    fn test_analyze_file_diff_modify_function() {
        let old_code = r#"
pub fn calculate_tax(amount: f64) -> f64 {
    amount * 0.1
}
"#;
        let new_code = r#"
pub fn calculate_tax(amount: f64) -> f64 {
    amount * 0.15
}
"#;
        let events = analyze_file_diff(Path::new("src/billing.rs"), Some(old_code), Some(new_code));
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EditKind::FileModified);
        assert_eq!(events[1].kind, EditKind::FunctionModified);
        assert_eq!(events[1].symbol.as_deref(), Some("calculate_tax"));
    }

    #[test]
    fn test_analyze_file_diff_delete_class_and_methods() {
        let old_code = r#"
pub struct LegacySession;

impl LegacySession {
    pub fn init() {}
    pub fn validate() {}
    pub fn close() {}
}
"#;
        let new_code = r#"
// Legacy session removed
"#;
        let events = analyze_file_diff(Path::new("src/legacy.rs"), Some(old_code), Some(new_code));
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].kind, EditKind::FileModified);
        assert_eq!(events[1].kind, EditKind::ClassDeleted);
        assert_eq!(events[1].symbol.as_deref(), Some("LegacySession"));
        assert_eq!(events[2].kind, EditKind::FunctionDeleted);
        assert_eq!(events[2].symbol.as_deref(), Some("init"));
        assert_eq!(events[3].kind, EditKind::FunctionDeleted);
        assert_eq!(events[3].symbol.as_deref(), Some("validate"));
        assert_eq!(events[4].kind, EditKind::FunctionDeleted);
        assert_eq!(events[4].symbol.as_deref(), Some("close"));
    }

    #[test]
    fn test_analyze_file_diff_delete_file() {
        let old_code = r#"
pub struct OldWidget;

pub fn draw_widget() {}
"#;
        let events = analyze_file_diff(Path::new("src/old_widget.rs"), Some(old_code), None);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].kind, EditKind::FileDeleted);
        assert_eq!(events[1].kind, EditKind::ClassDeleted);
        assert_eq!(events[1].symbol.as_deref(), Some("OldWidget"));
        assert_eq!(events[2].kind, EditKind::FunctionDeleted);
        assert_eq!(events[2].symbol.as_deref(), Some("draw_widget"));
    }

    #[test]
    fn test_unchanged_file_returns_no_events() {
        let code = "pub fn foo() {}";
        let events = analyze_file_diff(Path::new("src/lib.rs"), Some(code), Some(code));
        assert!(events.is_empty());
    }

    #[test]
    fn test_analyze_typescript_file_diff() {
        let old_ts = r#"
export class Router {
    navigate(to: string) {
        console.log(to);
    }
}
"#;
        let new_ts = r#"
export class Router {
    navigate(to: string) {
        window.location.href = to;
    }
    back() {
        window.history.back();
    }
}
"#;
        let events = analyze_file_diff(Path::new("src/router.ts"), Some(old_ts), Some(new_ts));
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].kind, EditKind::FileModified);
        assert_eq!(events[1].kind, EditKind::FunctionModified);
        assert_eq!(events[1].symbol.as_deref(), Some("navigate"));
        assert_eq!(events[2].kind, EditKind::FunctionAdded);
        assert_eq!(events[2].symbol.as_deref(), Some("back"));
    }

    #[test]
    fn test_analyze_python_file_diff() {
        let old_py = r#"
class Worker:
    def execute(self):
        print("old")
"#;
        let new_py = r#"
class Worker:
    def execute(self):
        print("new")
"#;
        let events = analyze_file_diff(Path::new("app/worker.py"), Some(old_py), Some(new_py));
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EditKind::FileModified);
        assert_eq!(events[1].kind, EditKind::FunctionModified);
        assert_eq!(events[1].symbol.as_deref(), Some("execute"));
    }

    #[test]
    fn test_analyze_csharp_file_diff() {
        let cs_code = r#"
public class PaymentGateway
{
    public void Charge() {}
}
"#;
        let events =
            analyze_file_diff(Path::new("Services/PaymentGateway.cs"), None, Some(cs_code));
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].kind, EditKind::FileAdded);
        assert_eq!(events[1].kind, EditKind::ClassAdded);
        assert_eq!(events[1].symbol.as_deref(), Some("PaymentGateway"));
        assert_eq!(events[2].kind, EditKind::FunctionAdded);
        assert_eq!(events[2].symbol.as_deref(), Some("Charge"));
    }

    #[test]
    fn test_analyze_go_file_diff() {
        let old_go = r#"
package server

type Server struct {
    port int
}

func (s *Server) Start() {
    println("start")
}

func Helper() {}
"#;
        let new_go = r#"
package server

type Server struct {
    port int
    host string
}

func (s *Server) Start() {
    println("start v2")
}

type Router struct {}
"#;
        let events = analyze_file_diff(Path::new("pkg/server.go"), Some(old_go), Some(new_go));
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].kind, EditKind::FileModified);
        assert_eq!(events[1].kind, EditKind::ClassModified);
        assert_eq!(events[1].symbol.as_deref(), Some("Server"));
        assert_eq!(events[2].kind, EditKind::FunctionModified);
        assert_eq!(events[2].symbol.as_deref(), Some("Start"));
        assert_eq!(events[3].kind, EditKind::ClassAdded);
        assert_eq!(events[3].symbol.as_deref(), Some("Router"));
        assert_eq!(events[4].kind, EditKind::FunctionDeleted);
        assert_eq!(events[4].symbol.as_deref(), Some("Helper"));
    }

    #[test]
    fn test_analyze_java_file_diff() {
        let old_java = r#"
public class PaymentProcessor {
    private int timeout = 10;

    public void process() {
        System.out.println("processing");
    }
}
"#;
        let new_java = r#"
public class PaymentProcessor {
    private int timeout = 30;

    public void process() {
        System.out.println("processing v2");
    }
}
"#;
        let events = analyze_file_diff(
            Path::new("src/PaymentProcessor.java"),
            Some(old_java),
            Some(new_java),
        );
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].kind, EditKind::FileModified);
        assert_eq!(events[1].kind, EditKind::ClassModified);
        assert_eq!(events[1].symbol.as_deref(), Some("PaymentProcessor"));
        assert_eq!(events[2].kind, EditKind::FunctionModified);
        assert_eq!(events[2].symbol.as_deref(), Some("process"));
    }

    #[test]
    fn test_analyze_cpp_file_diff() {
        let old_cpp = r#"
class Engine {
    int horsepower;
public:
    void start() {
        init();
    }
};
"#;
        let new_cpp = r#"
class Engine {
    int horsepower;
    int torque;
public:
    void start() {
        init_v2();
    }
};
"#;
        let events = analyze_file_diff(Path::new("src/engine.cpp"), Some(old_cpp), Some(new_cpp));
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].kind, EditKind::FileModified);
        assert_eq!(events[1].kind, EditKind::ClassModified);
        assert_eq!(events[1].symbol.as_deref(), Some("Engine"));
        assert_eq!(events[2].kind, EditKind::FunctionModified);
        assert_eq!(events[2].symbol.as_deref(), Some("start"));
    }

    #[test]
    fn test_analyze_ruby_file_diff() {
        let old_ruby = r#"
class PaymentGateway
  def charge(amount)
    puts "charge #{amount}"
  end
end
"#;
        let new_ruby = r#"
class PaymentGateway
  def charge(amount)
    puts "charge v2 #{amount}"
  end

  def refund(amount)
    puts "refund #{amount}"
  end
end
"#;
        let events = analyze_file_diff(
            Path::new("lib/payment_gateway.rb"),
            Some(old_ruby),
            Some(new_ruby),
        );
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].kind, EditKind::FileModified);
        assert_eq!(events[1].kind, EditKind::FunctionModified);
        assert_eq!(events[1].symbol.as_deref(), Some("charge"));
        assert_eq!(events[2].kind, EditKind::FunctionAdded);
        assert_eq!(events[2].symbol.as_deref(), Some("refund"));
    }

    #[test]
    fn test_analyze_php_file_diff() {
        let old_php = r#"<?php
class AuthService {
    public function login() {
        return true;
    }
}
"#;
        let new_php = r#"<?php
class AuthService {
    public function login() {
        return false;
    }
}

function verify_token() {
    return true;
}
"#;
        let events = analyze_file_diff(
            Path::new("src/AuthService.php"),
            Some(old_php),
            Some(new_php),
        );
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].kind, EditKind::FileModified);
        assert_eq!(events[1].kind, EditKind::FunctionModified);
        assert_eq!(events[1].symbol.as_deref(), Some("login"));
        assert_eq!(events[2].kind, EditKind::FunctionAdded);
        assert_eq!(events[2].symbol.as_deref(), Some("verify_token"));
    }

    #[test]
    fn test_analyze_swift_file_diff() {
        let old_swift = r#"
struct Configuration {
    var timeout: Int
}

class NetworkClient {
    func connect() {
        print("connecting")
    }
}
"#;
        let new_swift = r#"
struct Configuration {
    var timeout: Int
    var retries: Int
}

class NetworkClient {
    func connect() {
        print("connecting v2")
    }
}

actor ConnectionPool {}
"#;
        let events = analyze_file_diff(
            Path::new("Sources/Client.swift"),
            Some(old_swift),
            Some(new_swift),
        );
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].kind, EditKind::FileModified);
        assert_eq!(events[1].kind, EditKind::ClassModified);
        assert_eq!(events[1].symbol.as_deref(), Some("Configuration"));
        assert_eq!(events[2].kind, EditKind::FunctionModified);
        assert_eq!(events[2].symbol.as_deref(), Some("connect"));
        assert_eq!(events[3].kind, EditKind::ClassAdded);
        assert_eq!(events[3].symbol.as_deref(), Some("ConnectionPool"));
    }
}

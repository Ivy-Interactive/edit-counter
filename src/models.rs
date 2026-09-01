use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

impl EditKind {
    pub fn into_event(self, symbol: Option<&str>, file: &str, line: Option<usize>) -> EditEvent {
        EditEvent {
            kind: self,
            symbol: symbol.map(|s| s.to_string()),
            file: PathBuf::from(file),
            line,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditEvent {
    pub kind: EditKind,
    pub symbol: Option<String>,
    pub file: PathBuf,
    pub line: Option<usize>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

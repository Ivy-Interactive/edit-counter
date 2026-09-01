# Edit Counter (`edit-counter`)

A fast Rust CLI tool and algorithm that tracks and counts semantic code edits (new classes, functions, files, modifications, and deletions) to provide accurate edit metrics for AI coding agents and developer workflows.

## Features

- **Semantic Edit Tracking**: Counts changes at the semantic unit level (files, classes/structs, functions/methods, implementations, and deletions) rather than just raw line diffs.
- **Agent-Friendly Metrics**: Outputs structured JSON metrics ideal for agent evaluation, benching, cost calculation, and progress tracking.
- **Fast Rust Implementation**: Designed to run blazingly fast in CI pipelines, agent worktrees, and local developer environments.

## What is an Edit?

Traditional diff tools measure lines added or removed, which can be noisy (e.g. formatting passes, reordered imports, or multi-line comments). In contrast, `edit-counter` measures structural semantic changes.

Each distinct semantic event counts as exactly **1 edit**:

### 1. File Operations (1 edit each)
- `FileAdded`: Creating a brand new file on disk.
- `FileModified`: Modifying any existing file content.
- `FileDeleted`: Removing an existing file from disk.

### 2. Class / Type Operations (1 edit each)
Applies to structs, classes, traits, enums, interfaces, and record types:
- `ClassAdded`: Defining a new class, struct, trait, or enum.
- `ClassModified`: Changing signature, fields, or type definitions of an existing type.
- `ClassDeleted`: Deleting an existing type definition.

### 3. Function / Method Operations (1 edit each)
Applies to standalone functions, class/trait methods, and constructors:
- `FunctionAdded`: Introducing a new function or method.
- `FunctionModified`: Changing signature, logic, or docstrings of an existing function.
- `FunctionDeleted`: Removing a function or method.

---

## Counting Examples

Here are concrete developer scenarios demonstrating how edit events combine to produce the total edit count:

### Example 1: Creating a New Feature File
*Scenario*: Adding `src/auth.rs` containing a `UserAuth` struct and 2 helper methods (`login`, `logout`).
- 1 x `FileAdded` (`src/auth.rs`)
- 1 x `ClassAdded` (`UserAuth`)
- 2 x `FunctionAdded` (`login`, `logout`)
- **Total Edits**: 4

### Example 2: Refactoring an Existing Function
*Scenario*: Editing `calculate_tax` in `src/billing.rs` to support new discount rules.
- 1 x `FileModified` (`src/billing.rs`)
- 1 x `FunctionModified` (`calculate_tax`)
- **Total Edits**: 2

### Example 3: Deleting a Deprecated Class and Methods
*Scenario*: Modifying `src/legacy.rs` to delete `LegacySession` and its 3 member methods (`init`, `validate`, `close`).
- 1 x `FileModified` (`src/legacy.rs`)
- 1 x `ClassDeleted` (`LegacySession`)
- 3 x `FunctionDeleted` (`init`, `validate`, `close`)
- **Total Edits**: 5

### Example 4: Deleting an Entire File
*Scenario*: Deleting `src/old_widget.rs` which contained 1 struct (`OldWidget`) and 1 helper function (`draw_widget`).
- 1 x `FileDeleted` (`src/old_widget.rs`)
- 1 x `ClassDeleted` (`OldWidget`)
- 1 x `FunctionDeleted` (`draw_widget`)
- **Total Edits**: 3

---

## Example Output

### CLI Text Output
```text
$ edit-counter diff HEAD~1
Total edits: 4
```

### Structured JSON Output (`--json`)
When invoked with `--json`, `edit-counter` outputs a complete `EditReport` structure:

```json
{
  "total_edits": 4,
  "files_added": 1,
  "files_modified": 0,
  "files_deleted": 0,
  "classes_added": 1,
  "classes_modified": 0,
  "classes_deleted": 0,
  "functions_added": 2,
  "functions_modified": 0,
  "functions_deleted": 0,
  "events": [
    {
      "kind": "FileAdded",
      "symbol": null,
      "file": "src/auth.rs",
      "line": null
    },
    {
      "kind": "ClassAdded",
      "symbol": "UserAuth",
      "file": "src/auth.rs",
      "line": 1
    },
    {
      "kind": "FunctionAdded",
      "symbol": "login",
      "file": "src/auth.rs",
      "line": 10
    },
    {
      "kind": "FunctionAdded",
      "symbol": "logout",
      "file": "src/auth.rs",
      "line": 25
    }
  ]
}
```

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Analyze edits between commits or working tree
edit-counter diff HEAD~1

# Output metrics as JSON
edit-counter diff HEAD~1 --json

# Analyze specific files or directories
edit-counter analyze src/
```

## License

Licensed under the MIT License.

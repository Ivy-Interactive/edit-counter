# Edit Counter (`edit-counter`)

A fast Rust CLI tool and algorithm that tracks and counts semantic code edits (new classes, functions, files, modifications, and deletions) to provide accurate edit metrics for AI coding agents and developer workflows.

## Features

- **Semantic Edit Tracking**: Counts changes at the semantic unit level (files, classes/structs, functions/methods, implementations, and deletions) rather than just raw line diffs.
- **Agent-Friendly Metrics**: Outputs structured JSON metrics ideal for agent evaluation, benching, cost calculation, and progress tracking.
- **Fast Rust Implementation**: Designed to run blazingly fast in CI pipelines, agent worktrees, and local developer environments.

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

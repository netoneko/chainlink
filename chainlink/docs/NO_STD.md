# no_std Support for Chainlink

This document describes how to use the chainlink library in `no_std` environments.

## Overview

The chainlink library supports `no_std` environments through a pluggable output abstraction. This allows the same command logic to work in both standard library and no_std environments.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Commands Module                          │
│  (session.rs, list.rs, show.rs, etc.)                       │
│                                                              │
│  Uses out_println!(out, ...) instead of println!()          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Output Trait                              │
│                                                              │
│  pub trait Output {                                          │
│      fn print(&self, s: &str);                              │
│      fn println(&self, s: &str);                            │
│      fn eprint(&self, s: &str);                             │
│      fn eprintln(&self, s: &str);                           │
│  }                                                          │
└─────────────────────────────────────────────────────────────┘
              │                              │
              ▼                              ▼
┌─────────────────────────┐    ┌─────────────────────────────┐
│     StdOutput           │    │    Custom Implementation     │
│   (std feature)         │    │       (no_std)              │
│                         │    │                              │
│  Uses println!/print!   │    │  Routes to custom output    │
└─────────────────────────┘    └─────────────────────────────┘
```

## Using the Output Trait

### Standard Library (default)

When using chainlink with the default `std` feature, use `StdOutput`:

```rust
use chainlink::commands;
use chainlink::output::StdOutput;

fn main() {
    let db = get_database();
    let out = StdOutput;
    
    commands::list::run(&db, Some("open"), None, None, &out).unwrap();
    commands::session::start(&db, &out).unwrap();
}
```

### no_std Environment

In a `no_std` environment, implement the `Output` trait for your platform:

```rust
use chainlink::output::Output;

/// Output implementation for Akuma userspace
pub struct AkumaOutput;

impl Output for AkumaOutput {
    fn print(&self, s: &str) {
        libakuma::print(s);
    }
    
    fn println(&self, s: &str) {
        libakuma::print(s);
        libakuma::print("\n");
    }
    
    fn eprint(&self, s: &str) {
        // Route stderr to stdout in this environment
        libakuma::print(s);
    }
    
    fn eprintln(&self, s: &str) {
        libakuma::print(s);
        libakuma::print("\n");
    }
}

// Usage
fn run_command() {
    let db = get_database();
    let out = AkumaOutput;
    
    commands::session::start(&db, &out).unwrap();
}
```

## Convenience Macros

The library provides macros for formatted output:

```rust
use chainlink::{out_print, out_println, out_eprint, out_eprintln};
use chainlink::output::Output;

fn example(out: &impl Output) {
    out_println!(out, "Issue #{} created", 42);
    out_print!(out, "Processing...");
    out_eprintln!(out, "Warning: low priority");
}
```

## Feature Flags

| Feature | Description |
|---------|-------------|
| `std` (default) | Standard library support, enables `StdOutput` |
| `rusqlite-backend` | SQLite database backend using rusqlite |
| `cli` | Command-line interface (requires `std`) |

### Disabling std

To use chainlink without std:

```toml
[dependencies]
chainlink = { version = "0.1", default-features = false }
```

## Current Limitations

The commands module is currently feature-gated behind `std` because some commands use:

- `anyhow::Result` for error handling
- `std::fs` for file operations (init, export, import, etc.)
- `std::io` for user input (delete confirmation)

### Commands That Could Be no_std

These commands only use print operations and could work in no_std with a custom error type:

- `archive.rs` - Archive/unarchive issues
- `comment.rs` - Add comments
- `create.rs` - Create issues
- `deps.rs` - Dependency management
- `label.rs` - Label management
- `list.rs` - List issues
- `milestone.rs` - Milestone management
- `next.rs` - Get next issue
- `relate.rs` - Issue relationships
- `search.rs` - Search issues
- `session.rs` - Session management
- `show.rs` - Show issue details
- `timer.rs` - Time tracking
- `tree.rs` - Issue tree view
- `update.rs` - Update issues

### Commands That Require std

These commands use filesystem operations and must remain std-only:

- `delete.rs` - Uses stdin for confirmation
- `export.rs` - Writes to filesystem
- `import.rs` - Reads from filesystem
- `init.rs` - Creates directories and files
- `status.rs` - Reads/writes CHANGELOG
- `tested.rs` - Creates marker files

## Future Work

To make the print-only commands fully no_std compatible:

1. Replace `anyhow::Result` with a custom error type or use `DbError`
2. Feature-gate individual commands (std-only vs no_std compatible)
3. Consider using `chrono` with only the `alloc` feature for timestamps

## Example: Akuma Userspace

The Akuma operating system uses chainlink in its no_std userspace. See `userspace/chainlink/src/main.rs` for a complete example of:

- Custom `DatabaseBackend` implementation using sqld
- `AkumaOutput` implementation for the Output trait
- Manual command implementations that work around std limitations

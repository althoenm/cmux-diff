# cmux-diff

`cmux-diff` is a terminal UI for reviewing local, uncommitted Git changes.

It is built for the common "what changed in my working tree?" workflow, not for broad repository management. The default view stays focused on `HEAD` versus your current local state instead of showing every file changed across a branch or pull request.

## What It Does

- shows `Staged`, `Unstaged`, and `Untracked` files
- renders a unified diff for the selected file
- highlights added and removed lines inline
- stages and unstages files
- creates a commit from staged changes
- handles clean working trees with an explicit empty state

## Install

### Prerequisites

- `git`
- Rust stable with Cargo available on `PATH`

### Install with Cargo

```bash
cargo install --locked --git https://github.com/althoenm/cmux-diff
```

This installs the `cmux-diff` binary globally through Cargo.

Prebuilt binaries are not published yet.

### Run from source instead

```bash
git clone https://github.com/althoenm/cmux-diff
cd cmux-diff
cargo run --quiet
```

## Quick Start

### Run in the current repository

```bash
cmux-diff
```

### Run against a different repository

```bash
cmux-diff /path/to/repo
```

If the provided path is inside a Git repository, `cmux-diff` resolves the repository root automatically.

## What It Does Not Do

`cmux-diff` is intentionally narrow in scope today. It does not currently include:

- branch-wide review mode
- pull request or GitHub integration
- hunk-level staging
- discard or reset actions
- stash workflows
- side-by-side diff rendering
- commit graph browsing or remote management

## Why This Tool Exists

Many terminal Git tools are optimized to do everything. `cmux-diff` is optimized to make one workflow fast:

1. open a repository
2. see only what is locally dirty
3. inspect the diff
4. stage or unstage a file
5. write the commit

That focus keeps the UI small and avoids mixing local working-tree inspection with branch review or hosting-provider concerns.

## Interface

The current screen layout is:

- a header with repository, path, branch, and summary counts
- a left pane with `Staged`, `Unstaged`, and `Untracked` sections
- a right pane with the selected file's unified diff
- a commit input footer
- a status bar with the current action and key hints

## Keybindings

### Navigation

- `j` / `Down`: move selection down
- `k` / `Up`: move selection up
- `Tab`: switch focus between the file list and commit input
- `q`: quit

### Actions

- `r`: refresh working tree state
- `s`: stage the selected `Unstaged` or `Untracked` file
- `u`: unstage the selected `Staged` file
- `c`: focus the commit message input
- `Enter`: create a commit from staged changes when focused in the commit input
- `g`: create a commit from staged changes from the file list
- `Esc`: leave the commit input and return focus to the file list

## How It Works

`cmux-diff` shells out to the system `git` binary for repository operations. The current implementation uses commands such as:

- `git status --porcelain=v2 --branch`
- `git diff -- <path>`
- `git diff --cached -- <path>`
- `git diff --cached --root -- <path>`
- `git diff --no-index -- /dev/null <path>`
- `git add -- <path>`
- `git restore --staged -- <path>`
- `git commit -m <message>`

The application does not require GitHub credentials and does not call remote APIs.

## Development

### Project layout

- `src/git.rs`: subprocess-backed Git adapter and status parsing
- `src/app.rs`: application state and stage, unstage, and commit actions
- `src/ui.rs`: ratatui rendering
- `src/model.rs`: shared UI and domain types
- `tests/workflow.rs`: temp-repo integration tests

### Test

```bash
cargo test
```

### Format and check

```bash
cargo fmt
cargo check
```

## Security

The current design is intentionally conservative:

- the application operates on local repositories only
- Git commands are executed with `std::process::Command`, not shell interpolation
- file paths are passed as command arguments instead of being embedded in shell strings
- no tokens, API keys, or service credentials are required for the current feature set

The main trust boundary is the local `git` executable and the target repository contents. Future features such as discard actions, hunk editing, or remote-provider integration should be treated as higher-risk areas and designed accordingly.

If you discover a security issue, avoid posting exploit details in a public issue.

## Roadmap

Planned next steps:

1. improve diff scrolling and viewing ergonomics
2. add a separate branch review mode without changing the default local-changes workflow
3. add safer destructive actions with explicit confirmation
4. expand coverage for more complex repository states

## License

This project is licensed under Apache License 2.0. See `LICENSE` for the full text.

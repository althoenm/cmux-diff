# cmux-diff

`cmux-diff` is a terminal UI for the narrow workflow of reviewing **local uncommitted Git changes**.

The default view answers one question quickly:

> What is dirty in my working tree right now compared to `HEAD`?

This is intentionally smaller than a full Git client. It does **not** try to show every file changed on a branch by default, and it does **not** include GitHub or pull-request features in v1.

## Current Status

This repository is an early working prototype.

Implemented:

- `Staged`, `Unstaged`, and `Untracked` sections
- per-file diff view
- file-level stage / unstage
- commit message input and commit creation
- clean working-tree empty state
- test coverage for the main local-change workflows

Not implemented yet:

- branch review mode
- hunk-level staging
- discard/reset actions
- stash workflows
- PR / GitHub integration
- side-by-side diff rendering

## Why This Exists

Most terminal Git tools optimize for being broad. This project optimizes for one daily workflow:

1. open a repo
2. see only local changes
3. inspect a diff
4. stage or unstage a file
5. write a commit

That means the default mode deliberately avoids mixing in branch-wide review state.

## Features

### Local Changes View

The main screen is split into:

- header with repo root, branch, and summary counts
- left pane with `Staged`, `Unstaged`, and `Untracked` files
- right pane with the diff for the selected file
- commit footer for entering a commit message

### Git Operations

`cmux-diff` shells out to the system `git` binary for all repository operations.

Current commands used by the app include:

- `git status --porcelain=v2 --branch`
- `git diff -- <path>`
- `git diff --cached -- <path>`
- `git diff --cached --root -- <path>`
- `git diff --no-index -- /dev/null <path>`
- `git add -- <path>`
- `git restore --staged -- <path>`
- `git commit -m <message>`

No credentials or GitHub tokens are read by the app.

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

## Running Locally

### Prerequisites

- Rust stable
- `git` installed and available on `PATH`

The repo includes a `rust-toolchain.toml` pinned to stable, so a standard `rustup` setup is enough.

### Start the app

```bash
cargo run --quiet
```

To inspect a different repo:

```bash
cargo run --quiet -- /path/to/repo
```

If the provided path is inside a Git repo, `cmux-diff` resolves the repo root automatically.

## Development

### Layout

- `src/git.rs`: subprocess-backed Git adapter and `git status` parser
- `src/app.rs`: app state, selection logic, stage / unstage / commit actions
- `src/ui.rs`: ratatui rendering
- `src/model.rs`: shared UI and domain types
- `tests/workflow.rs`: temp-repo integration tests

### Test

```bash
cargo test
```

### Lint / format

```bash
cargo fmt
cargo check
```

## License

This project is licensed under Apache License 2.0.

See `LICENSE` for the full text.

## Security Notes

This repo is intentionally conservative:

- the app does not embed tokens, API keys, or service credentials
- Git commands are executed via `std::process::Command`, not shell interpolation
- file paths are passed as git arguments, which avoids shell-command injection
- the current app only works with the local filesystem and local Git repositories

Current review findings:

- no obvious secrets were found in the tracked source tree
- no network operations are performed by the application itself
- the main trust boundary is the local `git` executable and the target repository contents

Things to keep in mind for future work:

- if branch review or GitHub integration is added later, credentials must stay outside the repo and outside logs
- if discard/reset actions are added, they should require clear UX confirmation
- if hunk editing is added, diff parsing and patch application should be treated as a higher-risk area

## Non-Goals For v1

This project is not trying to replace a full Git desktop client yet.

Out of scope for the current version:

- commit graph browsing
- PR review comments
- issue tracking
- merge/rebase workflows
- remote repository management
- multi-repo dashboards

## Roadmap

Planned next steps:

1. improve the diff viewer behavior and scrolling
2. add a separate `Branch Review` mode without changing the default `Local Changes` view
3. add safer file actions such as discard/reset with confirmation
4. improve test coverage for selection behavior and more complex repository states

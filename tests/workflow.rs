use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use cmux_diff::app::AppState;
use cmux_diff::model::{ChangeSection, StatusLevel};
use tempfile::TempDir;

fn init_repo() -> Result<TempDir> {
    let temp = tempfile::tempdir()?;
    git(temp.path(), ["init", "--initial-branch=main"])?;
    git(temp.path(), ["config", "user.name", "Test User"])?;
    git(temp.path(), ["config", "user.email", "test@example.com"])?;
    Ok(temp)
}

fn git<I, S>(path: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let output = Command::new("git")
        .current_dir(path)
        .args(args.into_iter().map(|arg| arg.as_ref().to_string()))
        .output()
        .context("failed to run git in test repo")?;
    if !output.status.success() {
        anyhow::bail!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim().to_string()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[test]
fn app_state_shows_local_changes() -> Result<()> {
    let repo = init_repo()?;
    fs::write(repo.path().join("tracked.txt"), "hello\n")?;
    git(repo.path(), ["add", "--", "tracked.txt"])?;
    git(repo.path(), ["commit", "-m", "initial"])?;

    fs::write(repo.path().join("tracked.txt"), "hello\nworld\n")?;
    fs::write(repo.path().join("new.txt"), "brand new\n")?;

    let app = AppState::new(repo.path())?;
    assert_eq!(app.section_count(ChangeSection::Staged), 0);
    assert_eq!(app.section_count(ChangeSection::Unstaged), 1);
    assert_eq!(app.section_count(ChangeSection::Untracked), 1);
    assert!(app.diff.body.contains("tracked.txt") || app.diff.body.contains("+world"));
    Ok(())
}

#[test]
fn stage_and_unstage_updates_sections() -> Result<()> {
    let repo = init_repo()?;
    fs::write(repo.path().join("tracked.txt"), "hello\n")?;
    git(repo.path(), ["add", "--", "tracked.txt"])?;
    git(repo.path(), ["commit", "-m", "initial"])?;

    fs::write(repo.path().join("tracked.txt"), "hello\nworld\n")?;
    let mut app = AppState::new(repo.path())?;
    assert_eq!(app.section_count(ChangeSection::Unstaged), 1);

    app.stage_selected()?;
    assert_eq!(app.section_count(ChangeSection::Staged), 1);
    assert_eq!(app.section_count(ChangeSection::Unstaged), 0);

    app.unstage_selected()?;
    assert_eq!(app.section_count(ChangeSection::Staged), 0);
    assert_eq!(app.section_count(ChangeSection::Unstaged), 1);
    Ok(())
}

#[test]
fn commit_clears_staged_changes() -> Result<()> {
    let repo = init_repo()?;
    fs::write(repo.path().join("tracked.txt"), "hello\n")?;
    git(repo.path(), ["add", "--", "tracked.txt"])?;
    git(repo.path(), ["commit", "-m", "initial"])?;

    fs::write(repo.path().join("tracked.txt"), "hello\nworld\n")?;
    let mut app = AppState::new(repo.path())?;
    app.stage_selected()?;
    app.commit.message = "add world".to_string();
    app.commit()?;

    assert_eq!(app.section_count(ChangeSection::Staged), 0);
    assert_eq!(app.section_count(ChangeSection::Unstaged), 0);
    assert_eq!(app.section_count(ChangeSection::Untracked), 0);
    assert!(app.commit.message.is_empty());
    Ok(())
}

#[test]
fn clean_repo_shows_empty_state() -> Result<()> {
    let repo = init_repo()?;
    fs::write(repo.path().join("tracked.txt"), "hello\n")?;
    git(repo.path(), ["add", "--", "tracked.txt"])?;
    git(repo.path(), ["commit", "-m", "initial"])?;

    let app = AppState::new(repo.path())?;
    assert_eq!(app.section_count(ChangeSection::Staged), 0);
    assert_eq!(app.section_count(ChangeSection::Unstaged), 0);
    assert_eq!(app.section_count(ChangeSection::Untracked), 0);
    assert!(app.selected_entry().is_none());
    assert!(app.diff.body.contains("Working tree is clean"));
    Ok(())
}

#[test]
fn stage_selected_handles_untracked_files() -> Result<()> {
    let repo = init_repo()?;
    fs::write(repo.path().join("tracked.txt"), "hello\n")?;
    git(repo.path(), ["add", "--", "tracked.txt"])?;
    git(repo.path(), ["commit", "-m", "initial"])?;
    fs::write(repo.path().join("new.txt"), "brand new\n")?;

    let mut app = AppState::new(repo.path())?;
    while app.selected_entry().map(|entry| entry.section) != Some(ChangeSection::Untracked) {
        app.move_selection(1)?;
    }

    app.stage_selected()?;
    assert_eq!(app.section_count(ChangeSection::Untracked), 0);
    assert_eq!(app.section_count(ChangeSection::Staged), 1);
    assert_eq!(
        app.selected_entry().map(|entry| entry.section),
        Some(ChangeSection::Staged)
    );
    assert!(app.diff.body.contains("new file mode") || app.diff.body.contains("+++ b/new.txt"));
    Ok(())
}

#[test]
fn commit_requires_message() -> Result<()> {
    let repo = init_repo()?;
    fs::write(repo.path().join("tracked.txt"), "hello\n")?;
    git(repo.path(), ["add", "--", "tracked.txt"])?;
    git(repo.path(), ["commit", "-m", "initial"])?;
    fs::write(repo.path().join("tracked.txt"), "hello\nworld\n")?;

    let mut app = AppState::new(repo.path())?;
    app.stage_selected()?;
    app.commit.message = "   ".to_string();
    app.commit()?;

    assert_eq!(app.status.level, StatusLevel::Error);
    assert!(app.status.text.contains("cannot be empty"));
    assert_eq!(app.section_count(ChangeSection::Staged), 1);
    Ok(())
}

#[test]
fn commit_requires_staged_changes() -> Result<()> {
    let repo = init_repo()?;
    fs::write(repo.path().join("tracked.txt"), "hello\n")?;
    git(repo.path(), ["add", "--", "tracked.txt"])?;
    git(repo.path(), ["commit", "-m", "initial"])?;
    fs::write(repo.path().join("tracked.txt"), "hello\nworld\n")?;

    let mut app = AppState::new(repo.path())?;
    app.commit.message = "add world".to_string();
    app.commit()?;

    assert_eq!(app.status.level, StatusLevel::Error);
    assert!(app.status.text.contains("No staged changes"));
    assert_eq!(app.section_count(ChangeSection::Unstaged), 1);
    assert_eq!(
        git(repo.path(), ["rev-list", "--count", "HEAD"])?.trim(),
        "1"
    );
    Ok(())
}

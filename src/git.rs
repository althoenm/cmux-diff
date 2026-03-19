use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use thiserror::Error;

use crate::diff::parse_diff_hunks;
use crate::model::{ChangeEntry, ChangeSection, DiffContent, StatusSnapshot};

#[derive(Debug, Error)]
pub enum GitError {
    #[error("{0}")]
    CommandFailed(String),
}

#[derive(Clone, Debug)]
pub struct GitClient {
    repo_root: PathBuf,
}

impl GitClient {
    pub fn discover(path: &Path) -> Result<Self> {
        let repo_root = run_git(path, ["rev-parse", "--show-toplevel"])?
            .trim()
            .to_string();
        let repo_root = PathBuf::from(repo_root);
        Ok(Self {
            repo_root: repo_root.canonicalize().unwrap_or(repo_root),
        })
    }

    pub fn load_status(&self) -> Result<StatusSnapshot> {
        let output = self.run(["status", "--porcelain=v2", "--branch"])?;
        let has_commits = self.has_commits()?;
        let repo_name = self
            .repo_root
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("repo")
            .to_string();
        let (branch, mut entries) = parse_status_output(&output);
        for entry in &mut entries {
            let (additions, deletions) = self.diff_stats_for_entry(entry, has_commits)?;
            entry.additions = additions;
            entry.deletions = deletions;
        }

        Ok(StatusSnapshot {
            repo_root: self.repo_root.clone(),
            repo_name,
            branch,
            has_commits,
            entries,
        })
    }

    pub fn diff_for_entry(&self, entry: &ChangeEntry, has_commits: bool) -> Result<DiffContent> {
        let title = format!("{} · {}", entry.section.title(), entry.display_path());
        let body = match entry.section {
            ChangeSection::Staged => {
                let output = if has_commits {
                    self.run(["diff", "--cached", "--", entry.path.as_str()])?
                } else {
                    self.run(["diff", "--cached", "--root", "--", entry.path.as_str()])?
                };
                empty_diff_fallback(output, "No staged diff for the selected file.")
            }
            ChangeSection::Unstaged => {
                let output = self.run(["diff", "--", entry.path.as_str()])?;
                empty_diff_fallback(output, "No unstaged diff for the selected file.")
            }
            ChangeSection::Untracked => {
                let output = self.run_allow_exit(
                    ["diff", "--no-index", "--", "/dev/null", entry.path.as_str()],
                    &[0, 1],
                )?;
                empty_diff_fallback(output, "Untracked file preview is not available.")
            }
        };

        let hunks = parse_diff_hunks(&body);
        Ok(DiffContent { title, body, hunks })
    }

    pub fn stage_file(&self, path: &str) -> Result<()> {
        self.run(["add", "--", path]).map(|_| ())
    }

    pub fn unstage_file(&self, path: &str) -> Result<()> {
        self.run(["restore", "--staged", "--", path]).map(|_| ())
    }

    pub fn commit(&self, message: &str) -> Result<()> {
        self.run(["commit", "-m", message]).map(|_| ())
    }

    pub fn discard_file(&self, entry: &ChangeEntry, has_commits: bool) -> Result<()> {
        match entry.section {
            ChangeSection::Staged => {
                if has_commits {
                    self.run([
                        "restore",
                        "--source=HEAD",
                        "--staged",
                        "--worktree",
                        "--",
                        entry.path.as_str(),
                    ])
                    .map(|_| ())
                } else {
                    self.run(["rm", "-f", "--cached", "--", entry.path.as_str()])
                        .map(|_| ())?;
                    self.run_allow_exit(["clean", "-f", "-d", "--", entry.path.as_str()], &[0, 1])
                        .map(|_| ())
                }
            }
            ChangeSection::Unstaged | ChangeSection::Untracked => {
                self.delete_worktree_path(&entry.path)
            }
        }
    }

    fn delete_worktree_path(&self, path: &str) -> Result<()> {
        let full_path = self.repo_root.join(path);
        match fs::symlink_metadata(&full_path) {
            Ok(metadata) => {
                if metadata.is_dir() {
                    fs::remove_dir_all(&full_path)
                        .with_context(|| format!("failed to delete {}", full_path.display()))
                } else {
                    fs::remove_file(&full_path)
                        .with_context(|| format!("failed to delete {}", full_path.display()))
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => {
                Err(error).with_context(|| format!("failed to access {}", full_path.display()))
            }
        }
    }

    fn has_commits(&self) -> Result<bool> {
        let mut command = Command::new("git");
        command
            .current_dir(&self.repo_root)
            .args(["rev-parse", "--verify", "HEAD"]);
        let output =
            command_output_with_retry(&mut command, "failed to check whether HEAD exists")?;
        Ok(output.status.success())
    }

    fn diff_stats_for_entry(
        &self,
        entry: &ChangeEntry,
        has_commits: bool,
    ) -> Result<(usize, usize)> {
        let output = match entry.section {
            ChangeSection::Staged => {
                if has_commits {
                    self.run(["diff", "--cached", "--numstat", "--", entry.path.as_str()])?
                } else {
                    self.run([
                        "diff",
                        "--cached",
                        "--root",
                        "--numstat",
                        "--",
                        entry.path.as_str(),
                    ])?
                }
            }
            ChangeSection::Unstaged => {
                self.run(["diff", "--numstat", "--", entry.path.as_str()])?
            }
            ChangeSection::Untracked => self.run_allow_exit(
                [
                    "diff",
                    "--no-index",
                    "--numstat",
                    "--",
                    "/dev/null",
                    entry.path.as_str(),
                ],
                &[0, 1],
            )?,
        };

        Ok(parse_numstat_output(&output))
    }

    fn run<I, S>(&self, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        run_git_command(&self.repo_root, args, &[0])
    }

    fn run_allow_exit<I, S>(&self, args: I, allowed: &[i32]) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        run_git_command(&self.repo_root, args, allowed)
    }
}

fn run_git(path: &Path, args: impl IntoIterator<Item = &'static str>) -> Result<String> {
    run_git_command(path, args, &[0])
}

fn run_git_command<I, S>(path: &Path, args: I, allowed: &[i32]) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new("git");
    cmd.current_dir(path)
        .arg("--no-pager")
        .arg("-c")
        .arg("color.ui=never")
        .env("GIT_PAGER", "cat");
    cmd.args(args);

    let output = command_output_with_retry(&mut cmd, "failed to run git command")?;
    let code = output.status.code().unwrap_or(-1);
    if !allowed.contains(&code) {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let message = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("git exited with status {code}")
        };
        return Err(GitError::CommandFailed(message).into());
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn command_output_with_retry(command: &mut Command, context: &str) -> Result<Output> {
    let mut delay = Duration::from_millis(25);

    for attempt in 0..4 {
        match command.output() {
            Ok(output) => return Ok(output),
            Err(error) if is_retryable_spawn_error(&error) && attempt < 3 => {
                thread::sleep(delay);
                delay *= 2;
            }
            Err(error) => return Err(error).context(context.to_string()),
        }
    }

    unreachable!("retry loop always returns or errors")
}

fn is_retryable_spawn_error(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::WouldBlock || error.raw_os_error() == Some(35)
}

fn empty_diff_fallback(output: String, fallback: &str) -> String {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        output
    }
}

fn parse_status_output(output: &str) -> (String, Vec<ChangeEntry>) {
    let mut branch = "detached".to_string();
    let mut entries = Vec::new();

    for line in output.lines() {
        if line.starts_with("# branch.head ") {
            let value = line.trim_start_matches("# branch.head ").trim();
            branch = if value == "(detached)" {
                "detached".to_string()
            } else {
                value.to_string()
            };
            continue;
        }

        if let Some(path) = line.strip_prefix("? ") {
            entries.push(ChangeEntry {
                section: ChangeSection::Untracked,
                path: path.to_string(),
                original_path: None,
                staged_status: None,
                unstaged_status: None,
                additions: 0,
                deletions: 0,
            });
            continue;
        }

        if let Some(rest) = line.strip_prefix("1 ") {
            if let Some((xy, path)) = parse_ordinary_record(rest) {
                entries.extend(status_entries_from_xy(xy, path, None));
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("2 ") {
            if let Some((xy, path, original_path)) = parse_rename_record(rest) {
                entries.extend(status_entries_from_xy(xy, path, Some(original_path)));
            }
        }
    }

    entries.sort_by(|left, right| {
        left.section
            .cmp(&right.section)
            .then_with(|| left.path.cmp(&right.path))
    });

    (branch, entries)
}

fn parse_ordinary_record(rest: &str) -> Option<(&str, String)> {
    let mut parts = rest.splitn(8, ' ');
    let xy = parts.next()?;
    for _ in 0..6 {
        parts.next()?;
    }
    let path = parts.next()?.to_string();
    Some((xy, path))
}

fn parse_rename_record(rest: &str) -> Option<(&str, String, String)> {
    let mut parts = rest.splitn(9, ' ');
    let xy = parts.next()?;
    for _ in 0..7 {
        parts.next()?;
    }
    let path_block = parts.next()?;
    let mut path_parts = path_block.splitn(2, '\t');
    let path = path_parts.next()?.to_string();
    let original_path = path_parts.next()?.to_string();
    Some((xy, path, original_path))
}

fn status_entries_from_xy(
    xy: &str,
    path: String,
    original_path: Option<String>,
) -> Vec<ChangeEntry> {
    let mut entries = Vec::new();
    let mut chars = xy.chars();
    let staged = chars.next().unwrap_or('.');
    let unstaged = chars.next().unwrap_or('.');

    if staged != '.' {
        entries.push(ChangeEntry {
            section: ChangeSection::Staged,
            path: path.clone(),
            original_path: original_path.clone(),
            staged_status: Some(staged),
            unstaged_status: None,
            additions: 0,
            deletions: 0,
        });
    }

    if unstaged != '.' {
        entries.push(ChangeEntry {
            section: ChangeSection::Unstaged,
            path,
            original_path,
            staged_status: None,
            unstaged_status: Some(unstaged),
            additions: 0,
            deletions: 0,
        });
    }

    entries
}

fn parse_numstat_output(output: &str) -> (usize, usize) {
    output
        .lines()
        .find_map(parse_numstat_line)
        .unwrap_or((0, 0))
}

fn parse_numstat_line(line: &str) -> Option<(usize, usize)> {
    let mut parts = line.splitn(3, '\t');
    let additions = parts.next()?;
    let deletions = parts.next()?;
    parts.next()?;

    Some((
        parse_numstat_value(additions),
        parse_numstat_value(deletions),
    ))
}

fn parse_numstat_value(value: &str) -> usize {
    value.parse::<usize>().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_lines_into_sections() {
        let input = "\
# branch.oid abcdef
# branch.head main
1 MM N... 100644 100644 100644 abc def src/main.rs
1 .M N... 100644 100644 100644 abc def Cargo.toml
? notes.txt
";

        let (branch, entries) = parse_status_output(input);
        assert_eq!(branch, "main");
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].section, ChangeSection::Staged);
        assert_eq!(entries[0].path, "src/main.rs");
        assert_eq!(entries[1].section, ChangeSection::Unstaged);
        assert_eq!(entries[1].path, "Cargo.toml");
        assert_eq!(entries[2].section, ChangeSection::Unstaged);
        assert_eq!(entries[2].path, "src/main.rs");
        assert_eq!(entries[3].section, ChangeSection::Untracked);
    }

    #[test]
    fn parses_rename_record_display_data() {
        let input = "\
# branch.head feature
2 R. N... 100644 100644 100644 abc def R100 src/new.rs\tsrc/old.rs
";

        let (_, entries) = parse_status_output(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].section, ChangeSection::Staged);
        assert_eq!(entries[0].path, "src/new.rs");
        assert_eq!(entries[0].original_path.as_deref(), Some("src/old.rs"));
    }

    #[test]
    fn parses_numstat_output_for_text_and_binary_changes() {
        assert_eq!(parse_numstat_output("12\t3\tsrc/app.rs\n"), (12, 3));
        assert_eq!(parse_numstat_output("-\t-\tassets/logo.png\n"), (0, 0));
        assert_eq!(parse_numstat_output(""), (0, 0));
    }
}

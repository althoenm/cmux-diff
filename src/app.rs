use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::diff::{effective_diff_scroll, raw_line_index_for_scroll, scroll_for_raw_line};
use crate::editor;
use crate::git::GitClient;
use crate::model::{
    ChangeEntry, ChangeSection, DiffContent, FocusArea, StatusMessage, StatusSnapshot,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommitState {
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct AppState {
    git: GitClient,
    pub repo_name: String,
    pub repo_root: String,
    pub repo_root_path: PathBuf,
    pub branch: String,
    pub has_commits: bool,
    all_changes: Vec<ChangeEntry>,
    pub changes: Vec<ChangeEntry>,
    pub filter_query: String,
    pub diff_wrap: bool,
    pub diff: DiffContent,
    pub diff_scroll: u16,
    diff_viewport_width: usize,
    diff_viewport_height: usize,
    pub selected_entry_id: Option<String>,
    pub focus: FocusArea,
    pub commit: CommitState,
    pub status: StatusMessage,
}

impl AppState {
    pub fn new(path: &Path) -> Result<Self> {
        let git = GitClient::discover(path)?;
        let mut state = Self {
            git,
            repo_name: String::new(),
            repo_root: String::new(),
            repo_root_path: PathBuf::new(),
            branch: String::new(),
            has_commits: false,
            all_changes: Vec::new(),
            changes: Vec::new(),
            filter_query: String::new(),
            diff_wrap: true,
            diff: DiffContent::empty("Loading diff…"),
            diff_scroll: 0,
            diff_viewport_width: 1,
            diff_viewport_height: 1,
            selected_entry_id: None,
            focus: FocusArea::FileList,
            commit: CommitState::default(),
            status: StatusMessage::info("Ready."),
        };
        state.refresh(None)?;
        Ok(state)
    }

    pub fn refresh(&mut self, preferred: Option<(String, ChangeSection)>) -> Result<()> {
        let snapshot = self.git.load_status()?;
        self.apply_snapshot(snapshot, preferred)?;
        self.status = StatusMessage::info("Refreshed local changes.");
        Ok(())
    }

    pub fn set_diff_viewport(&mut self, width: usize, height: usize) {
        self.diff_viewport_width = width.max(1);
        self.diff_viewport_height = height.max(1);
        self.diff_scroll = self.clamp_diff_scroll(self.diff_scroll);
    }

    pub fn entries_in_section(&self, section: ChangeSection) -> impl Iterator<Item = &ChangeEntry> {
        self.changes
            .iter()
            .filter(move |entry| entry.section == section)
    }

    pub fn section_count(&self, section: ChangeSection) -> usize {
        self.entries_in_section(section).count()
    }

    pub fn total_section_count(&self, section: ChangeSection) -> usize {
        self.all_changes
            .iter()
            .filter(|entry| entry.section == section)
            .count()
    }

    pub fn total_change_count(&self) -> usize {
        self.all_changes.len()
    }

    pub fn is_filter_active(&self) -> bool {
        !self.filter_query.is_empty()
    }

    pub fn selected_entry(&self) -> Option<&ChangeEntry> {
        self.selected_entry_id
            .as_ref()
            .and_then(|id| self.changes.iter().find(|entry| entry.id() == *id))
    }

    pub fn move_selection(&mut self, delta: isize) -> Result<()> {
        if self.changes.is_empty() {
            return Ok(());
        }

        let current_index = self
            .selected_entry_id
            .as_ref()
            .and_then(|id| self.changes.iter().position(|entry| entry.id() == *id))
            .unwrap_or(0);
        let next_index = current_index
            .saturating_add_signed(delta)
            .min(self.changes.len() - 1);
        self.selected_entry_id = Some(self.changes[next_index].id());
        self.status = StatusMessage::info(self.selection_status_text());
        self.reload_selected_diff()
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            FocusArea::FileList => FocusArea::DiffView,
            FocusArea::DiffView => FocusArea::CommitInput,
            FocusArea::CommitInput => FocusArea::FilterInput,
            FocusArea::FilterInput => FocusArea::FileList,
        };
    }

    pub fn focus_commit(&mut self) {
        self.focus = FocusArea::CommitInput;
    }

    pub fn focus_file_list(&mut self) {
        self.focus = FocusArea::FileList;
        self.status = StatusMessage::info(self.selection_status_text());
    }

    pub fn focus_filter(&mut self) {
        self.focus = FocusArea::FilterInput;
        self.status =
            StatusMessage::info("Type to filter changed files. Esc returns to the file list.");
    }

    pub fn scroll_diff(&mut self, delta: i16) {
        let requested = self.diff_scroll.saturating_add_signed(delta);
        self.diff_scroll = self.clamp_diff_scroll(requested);
    }

    pub fn jump_to_next_hunk(&mut self) {
        self.jump_to_hunk(true);
    }

    pub fn jump_to_previous_hunk(&mut self) {
        self.jump_to_hunk(false);
    }

    pub fn toggle_diff_wrap(&mut self) {
        self.diff_wrap = !self.diff_wrap;
        self.diff_scroll = self.clamp_diff_scroll(self.diff_scroll);
        self.status = StatusMessage::info(if self.diff_wrap {
            "Enabled wrapped diff lines."
        } else {
            "Showing raw, unwrapped diff lines."
        });
    }

    pub fn stage_selected(&mut self) -> Result<()> {
        let Some(entry) = self.selected_entry().cloned() else {
            return Ok(());
        };
        match entry.section {
            ChangeSection::Unstaged | ChangeSection::Untracked => {
                self.git.stage_file(&entry.path)?;
                self.apply_post_action_refresh(Some((entry.path, ChangeSection::Staged)))?;
                self.status = StatusMessage::success("Staged selected file.");
                Ok(())
            }
            ChangeSection::Staged => {
                self.status = StatusMessage::info("Selected file is already staged.");
                Ok(())
            }
        }
    }

    pub fn unstage_selected(&mut self) -> Result<()> {
        let Some(entry) = self.selected_entry().cloned() else {
            return Ok(());
        };
        if entry.section != ChangeSection::Staged {
            self.status = StatusMessage::info("Selected file is not in the staged section.");
            return Ok(());
        }

        self.git.unstage_file(&entry.path)?;
        self.apply_post_action_refresh(Some((entry.path, ChangeSection::Unstaged)))?;
        self.status = StatusMessage::success("Unstaged selected file.");
        Ok(())
    }

    pub fn commit(&mut self) -> Result<()> {
        if self.commit.message.trim().is_empty() {
            self.status = StatusMessage::error("Commit message cannot be empty.");
            return Ok(());
        }
        if self.total_section_count(ChangeSection::Staged) == 0 {
            self.status = StatusMessage::error("No staged changes to commit.");
            return Ok(());
        }

        self.git
            .commit(self.commit.message.trim())
            .context("commit failed")?;
        self.commit.message.clear();
        self.focus = FocusArea::FileList;
        self.apply_post_action_refresh(None)?;
        self.status = StatusMessage::success("Created commit from staged changes.");
        Ok(())
    }

    pub fn discard_selected(&mut self) -> Result<()> {
        let Some(entry) = self.selected_entry().cloned() else {
            return Ok(());
        };

        self.git.discard_file(&entry, self.has_commits)?;
        self.apply_post_action_refresh(None)?;
        self.status = StatusMessage::success(match entry.section {
            ChangeSection::Staged => format!("Rolled back {} to HEAD.", entry.display_path()),
            ChangeSection::Unstaged | ChangeSection::Untracked => {
                format!("Deleted {} from the working tree.", entry.display_path())
            }
        });
        Ok(())
    }

    pub fn open_selected_in_editor(&mut self) -> Result<()> {
        let Some(entry) = self.selected_entry().cloned() else {
            self.status = StatusMessage::info("No file is selected.");
            return Ok(());
        };

        let line = self.current_hunk_line().or(Some(1));
        let path = self.repo_root_path.join(&entry.path);
        let message = editor::open_in_editor(&path, line)?;
        self.status = StatusMessage::success(message);
        Ok(())
    }

    pub fn push_commit_char(&mut self, ch: char) {
        self.commit.message.push(ch);
    }

    pub fn backspace_commit(&mut self) {
        self.commit.message.pop();
    }

    pub fn push_filter_char(&mut self, ch: char) -> Result<()> {
        self.filter_query.push(ch);
        self.reapply_filter(None)?;
        self.status = StatusMessage::info(self.filter_status_text());
        Ok(())
    }

    pub fn backspace_filter(&mut self) -> Result<()> {
        self.filter_query.pop();
        self.reapply_filter(None)?;
        self.status = StatusMessage::info(self.filter_status_text());
        Ok(())
    }

    pub fn clear_filter(&mut self) -> Result<()> {
        if self.filter_query.is_empty() {
            self.status = StatusMessage::info("Filter is already empty.");
            return Ok(());
        }

        self.filter_query.clear();
        self.reapply_filter(None)?;
        self.status = StatusMessage::info("Cleared file filter.");
        Ok(())
    }

    pub fn current_hunk_position(&self) -> Option<(usize, usize)> {
        let total = self.diff.hunks.len();
        if total == 0 {
            return None;
        }

        self.current_hunk_index().map(|index| (index + 1, total))
    }

    pub fn focus_label(&self) -> &'static str {
        match self.focus {
            FocusArea::FileList => "files",
            FocusArea::DiffView => "diff",
            FocusArea::CommitInput => "commit",
            FocusArea::FilterInput => "filter",
        }
    }

    fn selection_status_text(&self) -> String {
        match self.selected_entry().map(|entry| entry.section) {
            Some(ChangeSection::Staged) => "Staged file selected.".to_string(),
            Some(ChangeSection::Unstaged) => "Unstaged file selected.".to_string(),
            Some(ChangeSection::Untracked) => "Untracked file selected.".to_string(),
            None if self.is_filter_active() => {
                format!("No changed files match filter `{}`.", self.filter_query)
            }
            None => "Working tree is clean.".to_string(),
        }
    }

    fn filter_status_text(&self) -> String {
        if self.filter_query.is_empty() {
            return self.selection_status_text();
        }

        format!(
            "Showing {} of {} changed files for filter `{}`.",
            self.changes.len(),
            self.all_changes.len(),
            self.filter_query
        )
    }

    fn apply_post_action_refresh(
        &mut self,
        preferred: Option<(String, ChangeSection)>,
    ) -> Result<()> {
        let snapshot = self.git.load_status()?;
        self.apply_snapshot(snapshot, preferred)
    }

    fn apply_snapshot(
        &mut self,
        snapshot: StatusSnapshot,
        preferred: Option<(String, ChangeSection)>,
    ) -> Result<()> {
        self.repo_name = snapshot.repo_name;
        self.repo_root = snapshot.repo_root.display().to_string();
        self.repo_root_path = snapshot.repo_root;
        self.branch = snapshot.branch;
        self.has_commits = snapshot.has_commits;
        self.all_changes = snapshot.entries;
        self.reapply_filter(preferred)
    }

    fn reapply_filter(&mut self, preferred: Option<(String, ChangeSection)>) -> Result<()> {
        let current = preferred.or_else(|| {
            self.selected_entry()
                .map(|entry| (entry.path.clone(), entry.section))
        });

        self.changes = self
            .all_changes
            .iter()
            .filter(|entry| entry_matches_filter(entry, &self.filter_query))
            .cloned()
            .collect();

        self.selected_entry_id = current
            .and_then(|(path, section)| {
                self.changes
                    .iter()
                    .find(|entry| entry.path == path && entry.section == section)
                    .map(ChangeEntry::id)
            })
            .or_else(|| self.changes.first().map(ChangeEntry::id));

        self.reload_selected_diff()
    }

    fn reload_selected_diff(&mut self) -> Result<()> {
        self.diff_scroll = 0;
        self.diff = if let Some(entry) = self.selected_entry().cloned() {
            self.git.diff_for_entry(&entry, self.has_commits)?
        } else if self.all_changes.is_empty() {
            DiffContent::empty("Working tree is clean. Local changes will appear here.")
        } else {
            DiffContent::empty("No changed files match the current filter.")
        };
        self.diff_scroll = self.clamp_diff_scroll(self.diff_scroll);
        Ok(())
    }

    fn clamp_diff_scroll(&self, requested: u16) -> u16 {
        effective_diff_scroll(
            &self.diff.body,
            requested,
            self.diff_viewport_width,
            self.diff_viewport_height,
            self.diff_wrap,
        )
    }

    fn jump_to_hunk(&mut self, forward: bool) {
        let Some(current_index) = self.current_hunk_index() else {
            self.status = StatusMessage::info("No diff hunks are available for the selected file.");
            return;
        };

        let target = if forward {
            current_index
                .saturating_add(1)
                .min(self.diff.hunks.len() - 1)
        } else {
            current_index.saturating_sub(1)
        };
        if target == current_index {
            self.status = StatusMessage::info(if forward {
                "Already at the last diff hunk."
            } else {
                "Already at the first diff hunk."
            });
            return;
        }

        self.diff_scroll = self.clamp_diff_scroll(scroll_for_raw_line(
            &self.diff.body,
            self.diff.hunks[target].line_index,
            self.diff_viewport_width,
            self.diff_wrap,
        ));
        self.status = StatusMessage::info(format!(
            "Jumped to hunk {} of {}.",
            target + 1,
            self.diff.hunks.len()
        ));
    }

    fn current_hunk_index(&self) -> Option<usize> {
        if self.diff.hunks.is_empty() {
            return None;
        }

        let raw_line = raw_line_index_for_scroll(
            &self.diff.body,
            self.diff_scroll,
            self.diff_viewport_width,
            self.diff_wrap,
        );
        Some(
            self.diff
                .hunks
                .iter()
                .rposition(|hunk| hunk.line_index <= raw_line)
                .unwrap_or(0),
        )
    }

    fn current_hunk_line(&self) -> Option<usize> {
        self.current_hunk_index()
            .and_then(|index| self.diff.hunks.get(index))
            .map(|hunk| hunk.new_start)
    }
}

fn entry_matches_filter(entry: &ChangeEntry, query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return true;
    }

    let needle = trimmed.to_lowercase();
    entry.path.to_lowercase().contains(&needle)
        || entry
            .original_path
            .as_deref()
            .map(|path| path.to_lowercase().contains(&needle))
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DiffHunk;

    fn seeded_state() -> AppState {
        AppState {
            git: GitClient::discover(Path::new(".")).expect("git root"),
            repo_name: "cmux-diff".to_string(),
            repo_root: "/tmp/cmux-diff".to_string(),
            repo_root_path: PathBuf::from("/tmp/cmux-diff"),
            branch: "main".to_string(),
            has_commits: true,
            all_changes: vec![
                ChangeEntry {
                    section: ChangeSection::Unstaged,
                    path: "src/app.rs".to_string(),
                    original_path: None,
                    staged_status: None,
                    unstaged_status: Some('M'),
                    additions: 2,
                    deletions: 1,
                },
                ChangeEntry {
                    section: ChangeSection::Unstaged,
                    path: "src/ui.rs".to_string(),
                    original_path: None,
                    staged_status: None,
                    unstaged_status: Some('M'),
                    additions: 4,
                    deletions: 2,
                },
            ],
            changes: vec![
                ChangeEntry {
                    section: ChangeSection::Unstaged,
                    path: "src/app.rs".to_string(),
                    original_path: None,
                    staged_status: None,
                    unstaged_status: Some('M'),
                    additions: 2,
                    deletions: 1,
                },
                ChangeEntry {
                    section: ChangeSection::Unstaged,
                    path: "src/ui.rs".to_string(),
                    original_path: None,
                    staged_status: None,
                    unstaged_status: Some('M'),
                    additions: 4,
                    deletions: 2,
                },
            ],
            filter_query: String::new(),
            diff_wrap: true,
            diff: DiffContent {
                title: "Diff".to_string(),
                body: "\
diff --git a/src/app.rs b/src/app.rs
@@ -1,2 +3,4 @@
 line
@@ -9 +20,3 @@
 line
"
                .to_string(),
                hunks: vec![
                    DiffHunk {
                        line_index: 1,
                        new_start: 3,
                    },
                    DiffHunk {
                        line_index: 3,
                        new_start: 20,
                    },
                ],
            },
            diff_scroll: 0,
            diff_viewport_width: 40,
            diff_viewport_height: 2,
            selected_entry_id: Some("unstaged:src/app.rs".to_string()),
            focus: FocusArea::FileList,
            commit: CommitState::default(),
            status: StatusMessage::info("Ready."),
        }
    }

    #[test]
    fn filter_query_reduces_visible_entries() {
        let mut state = seeded_state();

        state.filter_query = "workflow".to_string();
        state.reapply_filter(None).unwrap();

        assert!(state.changes.is_empty());
        assert!(state.diff.body.contains("No changed files match"));
    }

    #[test]
    fn hunk_navigation_tracks_current_position() {
        let mut state = seeded_state();

        assert_eq!(state.current_hunk_position(), Some((1, 2)));
        state.jump_to_next_hunk();
        assert_eq!(state.current_hunk_position(), Some((2, 2)));
        state.jump_to_previous_hunk();
        assert_eq!(state.current_hunk_position(), Some((1, 2)));
    }
}

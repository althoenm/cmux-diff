use std::path::Path;

use anyhow::{Context, Result};

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
    pub branch: String,
    pub has_commits: bool,
    pub changes: Vec<ChangeEntry>,
    pub selected_entry_id: Option<String>,
    pub diff: DiffContent,
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
            branch: String::new(),
            has_commits: false,
            changes: Vec::new(),
            selected_entry_id: None,
            diff: DiffContent::empty("Loading diff…"),
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

    pub fn entries_in_section(&self, section: ChangeSection) -> impl Iterator<Item = &ChangeEntry> {
        self.changes
            .iter()
            .filter(move |entry| entry.section == section)
    }

    pub fn section_count(&self, section: ChangeSection) -> usize {
        self.entries_in_section(section).count()
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
            FocusArea::FileList => FocusArea::CommitInput,
            FocusArea::CommitInput => FocusArea::FileList,
        };
    }

    pub fn focus_commit(&mut self) {
        self.focus = FocusArea::CommitInput;
    }

    pub fn focus_file_list(&mut self) {
        self.focus = FocusArea::FileList;
        self.status = StatusMessage::info(self.selection_status_text());
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
        if self.section_count(ChangeSection::Staged) == 0 {
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

    pub fn push_commit_char(&mut self, ch: char) {
        self.commit.message.push(ch);
    }

    pub fn backspace_commit(&mut self) {
        self.commit.message.pop();
    }

    fn selection_status_text(&self) -> String {
        match self.selected_entry().map(|entry| entry.section) {
            Some(ChangeSection::Staged) => "Staged file selected.".to_string(),
            Some(ChangeSection::Unstaged) => "Unstaged file selected.".to_string(),
            Some(ChangeSection::Untracked) => "Untracked file selected.".to_string(),
            None => "Working tree is clean.".to_string(),
        }
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
        self.branch = snapshot.branch;
        self.has_commits = snapshot.has_commits;
        self.changes = snapshot.entries;

        self.selected_entry_id = preferred
            .and_then(|(path, section)| {
                self.changes
                    .iter()
                    .find(|entry| entry.path == path && entry.section == section)
                    .map(ChangeEntry::id)
            })
            .or_else(|| {
                self.selected_entry_id.as_ref().and_then(|existing| {
                    self.changes
                        .iter()
                        .find(|entry| entry.id() == *existing)
                        .map(ChangeEntry::id)
                })
            })
            .or_else(|| self.changes.first().map(ChangeEntry::id));

        self.reload_selected_diff()
    }

    fn reload_selected_diff(&mut self) -> Result<()> {
        self.diff = if let Some(entry) = self.selected_entry().cloned() {
            self.git.diff_for_entry(&entry, self.has_commits)?
        } else {
            DiffContent::empty("Working tree is clean. Local changes will appear here.")
        };
        Ok(())
    }
}

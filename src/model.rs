use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ChangeSection {
    Staged,
    Unstaged,
    Untracked,
}

impl ChangeSection {
    pub const ALL: [Self; 3] = [Self::Staged, Self::Unstaged, Self::Untracked];

    pub fn title(self) -> &'static str {
        match self {
            Self::Staged => "Staged",
            Self::Unstaged => "Unstaged",
            Self::Untracked => "Untracked",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangeEntry {
    pub section: ChangeSection,
    pub path: String,
    pub original_path: Option<String>,
    pub staged_status: Option<char>,
    pub unstaged_status: Option<char>,
    pub additions: usize,
    pub deletions: usize,
}

impl ChangeEntry {
    pub fn id(&self) -> String {
        format!("{}:{}", self.section, self.path)
    }

    pub fn display_path(&self) -> String {
        match &self.original_path {
            Some(original) => format!("{original} -> {}", self.path),
            None => self.path.clone(),
        }
    }

    pub fn file_name(&self) -> String {
        Path::new(&self.path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(self.path.as_str())
            .to_string()
    }

    pub fn parent_path(&self) -> Option<String> {
        let parent = Path::new(&self.path)
            .parent()
            .and_then(|path| path.to_str())
            .unwrap_or("");
        if parent.is_empty() {
            None
        } else {
            Some(parent.to_string())
        }
    }

    pub fn tree_depth(&self) -> usize {
        Path::new(&self.path).components().count().saturating_sub(1)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusSnapshot {
    pub repo_root: PathBuf,
    pub repo_name: String,
    pub branch: String,
    pub has_commits: bool,
    pub entries: Vec<ChangeEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffContent {
    pub title: String,
    pub body: String,
    pub hunks: Vec<DiffHunk>,
}

impl DiffContent {
    pub fn empty(message: impl Into<String>) -> Self {
        Self {
            title: "Diff".to_string(),
            body: message.into(),
            hunks: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffHunk {
    pub line_index: usize,
    pub new_start: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusArea {
    FileList,
    DiffView,
    CommitInput,
    FilterInput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusLevel {
    Info,
    Success,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusMessage {
    pub level: StatusLevel,
    pub text: String,
}

impl StatusMessage {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            level: StatusLevel::Info,
            text: text.into(),
        }
    }

    pub fn success(text: impl Into<String>) -> Self {
        Self {
            level: StatusLevel::Success,
            text: text.into(),
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            level: StatusLevel::Error,
            text: text.into(),
        }
    }
}

impl fmt::Display for ChangeSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Staged => "staged",
            Self::Unstaged => "unstaged",
            Self::Untracked => "untracked",
        };
        f.write_str(value)
    }
}

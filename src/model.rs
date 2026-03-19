use std::fmt;
use std::path::PathBuf;

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
}

impl DiffContent {
    pub fn empty(message: impl Into<String>) -> Self {
        Self {
            title: "Diff".to_string(),
            body: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusArea {
    FileList,
    CommitInput,
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

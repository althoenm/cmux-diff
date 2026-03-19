use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Result, bail};

#[derive(Clone, Debug, Eq, PartialEq)]
struct EditorLaunch {
    program: String,
    args: Vec<String>,
    label: String,
}

pub fn open_in_editor(path: &Path, line: Option<usize>) -> Result<String> {
    let launch = build_editor_launch(path, line)?;

    let mut command = Command::new(&launch.program);
    command
        .args(&launch.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.spawn()?;

    let location = line
        .map(|number| format!(":{}", number))
        .unwrap_or_default();
    Ok(format!(
        "Opened {}{} in {}.",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file"),
        location,
        launch.label
    ))
}

fn build_editor_launch(path: &Path, line: Option<usize>) -> Result<EditorLaunch> {
    let line = line.filter(|number| *number > 0);
    let absolute = path.canonicalize().unwrap_or_else(|_| PathBuf::from(path));
    let location = line
        .map(|number| format!("{}:{number}", absolute.display()))
        .unwrap_or_else(|| absolute.display().to_string());

    if let Some(value) = env::var_os("CMUX_DIFF_EDITOR")
        .or_else(|| env::var_os("VISUAL"))
        .or_else(|| env::var_os("EDITOR"))
    {
        let program = value
            .to_string_lossy()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        if !program.is_empty() {
            if let Some(launch) = build_known_editor_launch(&program, &absolute, line) {
                return Ok(launch);
            }
            bail!(
                "Unsupported editor command `{program}`. Use a GUI editor like `code` or unset EDITOR."
            );
        }
    }

    #[cfg(target_os = "macos")]
    {
        Ok(EditorLaunch {
            program: "open".to_string(),
            args: vec![location],
            label: "the default macOS app".to_string(),
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        bail!(
            "No supported editor was found. Set CMUX_DIFF_EDITOR to a GUI editor command such as `code`."
        )
    }
}

fn build_known_editor_launch(
    program: &str,
    absolute: &Path,
    line: Option<usize>,
) -> Option<EditorLaunch> {
    let basename = Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program);
    let location = line
        .map(|number| format!("{}:{number}", absolute.display()))
        .unwrap_or_else(|| absolute.display().to_string());

    match basename {
        "code" | "codium" | "cursor" | "windsurf" => Some(EditorLaunch {
            program: program.to_string(),
            args: vec!["--goto".to_string(), location],
            label: basename.to_string(),
        }),
        "zed" | "subl" | "mate" => Some(EditorLaunch {
            program: program.to_string(),
            args: vec![location],
            label: basename.to_string(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_code_style_goto_commands() {
        let path = Path::new("/tmp/project/src/main.rs");
        let launch = build_known_editor_launch("code", path, Some(14)).unwrap();

        assert_eq!(launch.program, "code");
        assert_eq!(launch.args, vec!["--goto", "/tmp/project/src/main.rs:14"]);
    }

    #[test]
    fn rejects_unknown_editor_commands() {
        assert!(build_known_editor_launch("nvim", Path::new("/tmp/main.rs"), Some(3)).is_none());
    }
}

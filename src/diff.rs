use crate::domain::ChangedPath;
use std::path::Path;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiffError {
    #[error("git diff failed: {0}")]
    Git(String),
    #[error("invalid git status line: {0}")]
    InvalidStatus(String),
}

pub trait DiffProvider {
    fn changed_paths(&self) -> Result<Vec<ChangedPath>, DiffError>;
}

#[derive(Debug, Clone)]
pub struct GitDiffProvider {
    pub root: std::path::PathBuf,
    pub base: String,
    pub head: String,
}

impl GitDiffProvider {
    pub fn new(root: impl AsRef<Path>, base: impl Into<String>, head: impl Into<String>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            base: base.into(),
            head: head.into(),
        }
    }
}

impl DiffProvider for GitDiffProvider {
    fn changed_paths(&self) -> Result<Vec<ChangedPath>, DiffError> {
        let output = Command::new("git")
            .current_dir(&self.root)
            .args([
                "diff",
                "--name-status",
                "--find-renames",
                &format!("{}..{}", self.base, self.head),
            ])
            .output()
            .map_err(|error| DiffError::Git(error.to_string()))?;
        if !output.status.success() {
            return Err(DiffError::Git(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        parse_name_status(&String::from_utf8_lossy(&output.stdout))
    }
}

pub fn parse_name_status(input: &str) -> Result<Vec<ChangedPath>, DiffError> {
    input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(parse_status_line)
        .collect()
}

fn parse_status_line(line: &str) -> Result<ChangedPath, DiffError> {
    let fields = line.split('\t').collect::<Vec<_>>();
    if fields.len() < 2 {
        return Err(DiffError::InvalidStatus(line.to_owned()));
    }
    let status = fields[0];
    match status.chars().next() {
        Some('R') | Some('C') if fields.len() >= 3 => Ok(ChangedPath {
            old_path: Some(fields[1].to_owned()),
            path: fields[2].to_owned(),
            is_new: false,
        }),
        Some('A' | 'M' | 'D' | 'T' | 'U') => Ok(ChangedPath {
            old_path: None,
            path: fields[1].to_owned(),
            is_new: status.starts_with('A'),
        }),
        _ => Err(DiffError::InvalidStatus(line.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_added_modified_deleted_and_renamed_files() {
        let result = parse_name_status(
            "A\tsrc/new.py\nM\tsrc/changed.ts\nD\told.ex\nR100\told.js\tnew.js\n",
        )
        .unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].path, "src/new.py");
        assert_eq!(result[3].old_path.as_deref(), Some("old.js"));
        assert_eq!(result[3].path, "new.js");
    }

    #[test]
    fn rejects_malformed_status() {
        assert!(matches!(
            parse_name_status("M"),
            Err(DiffError::InvalidStatus(_))
        ));
        assert!(matches!(
            parse_name_status("X\tfile"),
            Err(DiffError::InvalidStatus(_))
        ));
        assert!(matches!(
            parse_name_status("R100\told.py"),
            Err(DiffError::InvalidStatus(_))
        ));
    }

    #[test]
    fn reads_a_real_git_diff() {
        let directory = tempfile::tempdir().unwrap();
        for args in [
            &["init"][..],
            &["config", "user.email", "test@example.test"][..],
            &["config", "user.name", "Test"][..],
        ] {
            assert!(
                Command::new("git")
                    .current_dir(directory.path())
                    .args(args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        std::fs::write(directory.path().join("file.py"), "print(1)\n").unwrap();
        assert!(
            Command::new("git")
                .current_dir(directory.path())
                .args(["add", "."])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .current_dir(directory.path())
                .args(["commit", "-m", "initial"])
                .output()
                .unwrap()
                .status
                .success()
        );
        std::fs::write(directory.path().join("file.py"), "print(2)\n").unwrap();
        assert!(
            Command::new("git")
                .current_dir(directory.path())
                .args(["add", "."])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .current_dir(directory.path())
                .args(["commit", "-m", "change"])
                .output()
                .unwrap()
                .status
                .success()
        );
        let result = GitDiffProvider::new(directory.path(), "HEAD~1", "HEAD");
        assert_eq!(result.changed_paths().unwrap()[0].path, "file.py");
        let output = Command::new("git")
            .current_dir(directory.path())
            .args(["diff", "--name-status", "--find-renames", "HEAD~1..HEAD"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let parsed = parse_name_status(&String::from_utf8_lossy(&output.stdout)).unwrap();
        assert_eq!(parsed[0].path, "file.py");
    }
}

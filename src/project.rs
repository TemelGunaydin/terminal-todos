use crate::model::safe;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Capture the origin of a new task, not the directory from which it is later viewed.
pub fn current() -> Option<String> {
    std::env::current_dir().ok().and_then(|cwd| detect(&cwd))
}

pub fn detect(cwd: &Path) -> Option<String> {
    let root = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            PathBuf::from(String::from_utf8_lossy(&output.stdout).trim_end_matches(['\n', '\r']))
        })
        .filter(|root| root.is_absolute());
    name(root.as_deref().unwrap_or(cwd))
}

fn name(path: &Path) -> Option<String> {
    let name = safe(&path.file_name()?.to_string_lossy());
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// A stable palette shared by TUI badges and CLI labels; never persist presentation data.
pub fn rgb(project: &str) -> (u8, u8, u8) {
    const PALETTE: [(u8, u8, u8); 6] = [
        (125, 211, 252), // sky
        (196, 181, 253), // lavender
        (252, 211, 77),  // amber
        (253, 164, 175), // rose
        (165, 180, 252), // periwinkle
        (253, 186, 116), // peach
    ];
    let hash = project
        .as_bytes()
        .iter()
        .fold(2_166_136_261_u32, |hash, byte| {
            (hash ^ u32::from(*byte)).wrapping_mul(16_777_619)
        });
    PALETTE[hash as usize % PALETTE.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn nested_git_directory_uses_the_repository_name() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("Türkçe proje 🦀");
        std::fs::create_dir_all(root.join("src/nested"))?;
        assert!(
            Command::new("git")
                .args(["init", "-q"])
                .arg(&root)
                .status()?
                .success()
        );
        assert_eq!(
            detect(&root.join("src/nested")).as_deref(),
            Some("Türkçe proje 🦀")
        );
        Ok(())
    }

    #[test]
    fn non_git_directory_falls_back_without_terminal_controls() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("Notes\n\u{1b} project");
        std::fs::create_dir(&root)?;
        assert_eq!(detect(&root).as_deref(), Some("Notes   project"));
        assert_eq!(name(Path::new("/")), None);
        Ok(())
    }

    #[test]
    fn colors_are_repeatable_and_not_the_old_mint_accent() {
        for project in ["terminal-todos", "Bookfun", "Türkçe proje 🦀"] {
            assert_eq!(rgb(project), rgb(project));
            assert_ne!(rgb(project), (82, 219, 175));
        }
        assert_ne!(rgb("terminal-todos"), rgb("Bookfun"));
    }
}

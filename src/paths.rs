use crate::{flag, Res};
use std::path::{Path, PathBuf};
use std::process::Command;

const BASE_TEMPLATE: &str = include_str!("../templates/Memory.base");

pub fn vault_from_env() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_PLUGIN_OPTION_VAULT")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

pub fn data_dir() -> Res<PathBuf> {
    let dir = std::env::var_os("CLAUDE_PLUGIN_DATA")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or("CLAUDE_PLUGIN_DATA is not set: run this through the plugin's hooks")?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// The main checkout of the repository that holds `cwd`. Claude Code keys auto-memory on it,
/// so every worktree of a repo shares one memory folder.
pub fn repo_root(cwd: &Path) -> PathBuf {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let common = PathBuf::from(String::from_utf8_lossy(&o.stdout).trim());
            common
                .parent()
                .map_or_else(|| cwd.to_path_buf(), Path::to_path_buf)
        }
        _ => cwd.to_path_buf(),
    }
}

/// Claude Code's own memory folder for the repo. The notes live here, where Claude Code can
/// write them without a permission prompt. The vault only links to it.
pub fn memory_dir(cwd: &Path) -> Res<PathBuf> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home)
        .join(".claude/projects")
        .join(project_slug(&repo_root(cwd)))
        .join("memory"))
}

/// How Claude Code names a project's folder: every character but a letter or digit becomes `-`.
fn project_slug(root: &Path) -> String {
    root.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// The project's folder in the vault: a symlink to `memory_dir`, named after the repo.
pub fn vault_link(vault: &Path, cwd: &Path) -> PathBuf {
    let name = repo_root(cwd)
        .file_name()
        .map_or_else(|| "root".to_string(), |n| n.to_string_lossy().into_owned());
    vault.join(name)
}

pub fn is_linked(vault: &Path, cwd: &Path) -> bool {
    let (Ok(link), Ok(dir)) = (
        vault_link(vault, cwd).canonicalize(),
        memory_dir(cwd).and_then(|d| Ok(d.canonicalize()?)),
    ) else {
        return false;
    };
    link == dir
}

/// The memory folder `mem` commands work on: `--dir`, else the current repo's.
pub fn resolve_dir(args: &[&str]) -> Res<PathBuf> {
    match flag(args, "--dir") {
        Some(d) => Ok(PathBuf::from(d)),
        None => memory_dir(&std::env::current_dir()?),
    }
}

/// Links the project's memory folder into the vault and writes the vault's Base. It changes
/// no Claude Code setting, so every worktree and every new session picks it up as it is.
pub fn setup(args: &[&str]) -> Res {
    let vault = flag(args, "--vault")
        .map(PathBuf::from)
        .or_else(vault_from_env)
        .ok_or("setup needs --vault")?;
    let cwd = flag(args, "--cwd")
        .map(PathBuf::from)
        .map_or_else(std::env::current_dir, Ok)?;
    let dir = memory_dir(&cwd)?;
    std::fs::create_dir_all(&dir)?;
    std::fs::create_dir_all(&vault)?;
    let link = vault_link(&vault, &cwd);
    if !is_linked(&vault, &cwd) {
        if link.symlink_metadata().is_ok() {
            return Err(format!(
                "{} exists and is not a link to {}. Move its notes into that folder, remove it, and run setup again.",
                link.display(),
                dir.display()
            )
            .into());
        }
        std::os::unix::fs::symlink(&dir, &link)?;
    }
    let base = vault.join("Memory.base");
    let wrote_base = !base.exists();
    if wrote_base {
        std::fs::write(&base, BASE_TEMPLATE)?;
    }
    let report = serde_json::json!({
        "memory_dir": dir,
        "vault_link": link,
        "base": base,
        "wrote_base": wrote_base,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_match_claude_code() {
        assert_eq!(
            project_slug(Path::new("/Users/v/code/2501")),
            "-Users-v-code-2501"
        );
        assert_eq!(
            project_slug(Path::new("/Users/v/Library/Application Support/x.y")),
            "-Users-v-Library-Application-Support-x-y"
        );
    }
}

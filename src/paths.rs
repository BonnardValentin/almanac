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

/// The main checkout of the repository that holds `cwd`, so every worktree of a repo
/// shares one memory folder, the same way Claude Code's own auto-memory does.
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

pub fn memory_dir(vault: &Path, cwd: &Path) -> PathBuf {
    let root = repo_root(cwd);
    let name = root
        .file_name()
        .map_or_else(|| "root".to_string(), |n| n.to_string_lossy().into_owned());
    vault.join(name)
}

/// Where Claude Code keeps a project's memory when `autoMemoryDirectory` is not set.
pub fn default_memory_dir(cwd: &Path) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let slug = repo_root(cwd).to_string_lossy().replace(['/', '.'], "-");
    Some(
        PathBuf::from(home)
            .join(".claude/projects")
            .join(slug)
            .join("memory"),
    )
}

pub fn settings_file(cwd: &Path) -> PathBuf {
    repo_root(cwd).join(".claude/settings.local.json")
}

/// The memory folder `mem` commands work on: `--dir`, else the vault folder for the current repo.
pub fn resolve_dir(args: &[&str]) -> Res<PathBuf> {
    if let Some(d) = flag(args, "--dir") {
        return Ok(PathBuf::from(d));
    }
    let vault = flag(args, "--vault")
        .map(PathBuf::from)
        .or_else(vault_from_env)
        .ok_or("no memory folder: pass --dir, or --vault, or run inside the plugin")?;
    Ok(memory_dir(&vault, &std::env::current_dir()?))
}

pub fn configured_dir(cwd: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(settings_file(cwd)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v["autoMemoryDirectory"].as_str().map(PathBuf::from)
}

/// Creates the project's memory folder and the vault's Base. Changing Claude Code's
/// settings is left to the setup skill, which asks the user first.
pub fn setup(args: &[&str]) -> Res {
    let vault = PathBuf::from(flag(args, "--vault").ok_or("setup needs --vault")?);
    let cwd = flag(args, "--cwd")
        .map(PathBuf::from)
        .map_or_else(std::env::current_dir, Ok)?;
    let dir = memory_dir(&vault, &cwd);
    std::fs::create_dir_all(&dir)?;
    let base = vault.join("Memory.base");
    let wrote_base = !base.exists();
    if wrote_base {
        std::fs::write(&base, BASE_TEMPLATE)?;
    }
    let old = default_memory_dir(&cwd).filter(|d| d.is_dir() && *d != dir);
    let report = serde_json::json!({
        "memory_dir": dir,
        "settings_file": settings_file(&cwd),
        "configured": configured_dir(&cwd).as_deref() == Some(dir.as_path()),
        "base": base,
        "wrote_base": wrote_base,
        "old_memory_dir": old,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

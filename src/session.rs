use crate::log::now_secs;
use crate::paths::{data_dir, is_linked, memory_dir, vault_from_env};
use crate::{hook_input, mem, str_field, Res};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

const KEEP_SESSIONS_SECS: u64 = 7 * 86_400;

/// Per-session state the Stop hook needs: when the session started, how often it blocked,
/// and the repo's state at start, so a turn is judged on what this session changed.
pub struct Session {
    path: PathBuf,
    pub started: u64,
    pub blocks: u32,
    pub baseline: Option<u64>,
}

pub fn git(cwd: &Path, args: &[&str]) -> String {
    Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// Changes when the session edits, stages or commits anything in the repo.
pub fn work_fingerprint(cwd: &Path) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    git(cwd, &["rev-parse", "HEAD"]).hash(&mut h);
    git(cwd, &["status", "--porcelain"]).hash(&mut h);
    git(cwd, &["diff", "HEAD"]).hash(&mut h);
    h.finish()
}

impl Session {
    pub fn load(id: &str) -> Res<Session> {
        let dir = data_dir()?.join("sessions");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(id.replace(['/', '\\', '.'], "_"));
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let mut parts = text.split_whitespace().map(|s| s.parse::<u64>().ok());
        let started = parts.next().flatten().unwrap_or_else(now_secs);
        let blocks = parts.next().flatten().unwrap_or(0) as u32;
        let baseline = parts.next().flatten();
        Ok(Session {
            path,
            started,
            blocks,
            baseline,
        })
    }

    pub fn save(&self) -> Res {
        let baseline = self.baseline.map(|b| b.to_string()).unwrap_or_default();
        std::fs::write(
            &self.path,
            format!("{} {} {baseline}", self.started, self.blocks),
        )?;
        Ok(())
    }
}

fn prune_old_sessions(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let now = std::time::SystemTime::now();
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|age| age.as_secs() > KEEP_SESSIONS_SECS);
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// The memory folder of the session's project, or why there is none yet.
pub fn project_memory(cwd: &Path) -> Result<PathBuf, String> {
    let vault = vault_from_env()
        .ok_or("almanac: no vault is configured. Set one with /plugin configure almanac.")?;
    if !is_linked(&vault, cwd) {
        return Err(format!(
            "almanac: this project's memory is not in the vault yet. Run the almanac setup skill \
             (`almanac setup --vault {}`).",
            vault.display()
        ));
    }
    memory_dir(cwd).map_err(|e| e.to_string())
}

/// SessionStart: whatever it prints becomes context for the session.
pub fn start() -> Res {
    let input = hook_input()?;
    let cwd = Path::new(str_field(&input, "cwd")?);
    let mut session = Session::load(str_field(&input, "session_id")?)?;
    // A resumed or compacted session keeps its first baseline.
    if session.baseline.is_none() {
        session.baseline = Some(work_fingerprint(cwd));
    }
    session.save()?;
    if let Some(dir) = session.path.parent() {
        prune_old_sessions(dir);
    }
    match project_memory(cwd) {
        Ok(dir) => {
            if let Err(e) = mem::write_index(&dir) {
                println!("almanac: {e}");
            }
        }
        Err(msg) => println!("{msg}"),
    }
    Ok(())
}

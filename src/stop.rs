use crate::decide::{self, Stop};
use crate::jev::{self, Client, Kind, Question};
use crate::notes::INDEX_FILE;
use crate::session::{git, project_memory, work_fingerprint, Session};
use crate::{hook_input, mem, str_field, Res};
use std::path::Path;
use std::time::UNIX_EPOCH;

// A hook that can refuse forever is a hook that will.
const MAX_BLOCKS: u32 = 2;
const TRANSCRIPT_TAIL_BYTES: u64 = 4_000;

const REASON: &str =
    "Before you finish: did this session produce a lesson that a future session would want \
and that memory does not have yet? If yes, write it as one note with the almanac write-note skill. \
If no, say so in one line and stop.";

const QUESTIONS: &[Question] = &[Question {
    key: "memory_complete",
    kind: Kind::Noul,
    instructions: "The evidence shows what a coding session changed and how it ended. Answer yes if \
memory already holds every lesson from it that a future session would need, or if it produced no such \
lesson. Answer no if it found a cause, a constraint or a working method that is not written down.",
}];

fn wrote_note_since(dir: &Path, started: u64) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        let p = e.path();
        let is_note = p.extension().is_some_and(|x| x == "md")
            && p.file_name().is_some_and(|f| f != INDEX_FILE);
        let modified = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        is_note && modified >= started
    })
}

fn transcript_tail(path: &str) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(path) else {
        return String::new();
    };
    let len = f.metadata().map_or(0, |m| m.len());
    let _ = f.seek(SeekFrom::Start(len.saturating_sub(TRANSCRIPT_TAIL_BYTES)));
    let mut buf = Vec::new();
    let _ = f.read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

fn verdict(input: &serde_json::Value, cwd: &Path) -> Res<Stop> {
    // Without a decision model this is the scaffold: block, and let the agent judge.
    let Some(client) = Client::from_env() else {
        return Ok(Stop::Block);
    };
    let state = format!(
        "Changed files:\n{}\nEnd of the session transcript:\n{}",
        git(cwd, &["diff", "--stat", "HEAD"]),
        transcript_tail(input["transcript_path"].as_str().unwrap_or_default()),
    );
    let answers = client.ask(&state, QUESTIONS)?;
    Ok(decide::stop(jev::prob(&answers, "memory_complete")?))
}

pub fn run() -> Res {
    let input = hook_input()?;
    if input["stop_hook_active"] == true {
        return Ok(());
    }
    let cwd = Path::new(str_field(&input, "cwd")?);
    // Before setup, the SessionStart message already says what to do. Nagging on every stop would not help.
    let Ok(dir) = project_memory(cwd) else {
        return Ok(());
    };
    if let Err(e) = mem::write_index(&dir) {
        eprintln!("almanac: {e}");
    }
    let mut session = Session::load(str_field(&input, "session_id")?)?;
    let worked = session.baseline != Some(work_fingerprint(cwd));
    if session.blocks >= MAX_BLOCKS || !worked || wrote_note_since(&dir, session.started) {
        return Ok(());
    }
    match verdict(&input, cwd)? {
        Stop::Allow => {}
        Stop::AllowUnsure => println!(
            "{}",
            serde_json::json!({ "systemMessage": "almanac was not sure this session's lessons are in memory." })
        ),
        Stop::Block => {
            session.blocks += 1;
            session.save()?;
            println!(
                "{}",
                serde_json::json!({ "decision": "block", "reason": REASON })
            );
        }
    }
    Ok(())
}

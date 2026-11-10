use crate::date::Day;
use crate::Res;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const INDEX_FILE: &str = "MEMORY.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    Investigate,
    Verified,
    Rule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Active,
    Superseded,
}

#[derive(Clone, Debug)]
pub struct Note {
    pub path: PathBuf,
    pub file: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub stage: Stage,
    pub status: Status,
    pub verified_on: Option<Day>,
    pub mtime: SystemTime,
}

/// Frontmatter as (key, value) pairs. Nested keys are flattened to `parent.child`,
/// which is how notes written before almanac carry `metadata.type`.
pub fn frontmatter(text: &str) -> Vec<(String, String)> {
    let mut lines = text.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return Vec::new();
    }
    let mut fields = Vec::new();
    let mut parent = String::new();
    for line in lines {
        if line.trim_end() == "---" {
            return fields;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');
        let Some((key, value)) = line.trim().split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = unquote(value.trim());
        if !indented {
            parent = key.to_string();
            fields.push((key.to_string(), value.to_string()));
        } else if !parent.is_empty() {
            fields.push((format!("{parent}.{key}"), value.to_string()));
        }
    }
    Vec::new()
}

fn unquote(v: &str) -> &str {
    let quoted = v.len() >= 2
        && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')));
    if quoted {
        &v[1..v.len() - 1]
    } else {
        v
    }
}

/// Claude Code nests a memory note's fields under `metadata:` when it writes one, so a field
/// counts wherever it is: top level first, then under `metadata`.
fn get<'a>(fields: &'a [(String, String)], key: &str) -> Option<&'a str> {
    let nested = format!("metadata.{key}");
    let find = |want: &str| {
        fields
            .iter()
            .find(|(k, v)| k == want && !v.is_empty())
            .map(|(_, v)| v.as_str())
    };
    find(key).or_else(|| find(&nested))
}

pub fn parse(path: &Path, text: &str, mtime: SystemTime) -> Note {
    let fields = frontmatter(text);
    let file = path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = file.trim_end_matches(".md").to_string();
    // A note with no stage, or one we do not recognize, has not been checked by anything.
    let stage = match get(&fields, "stage") {
        Some("rule") => Stage::Rule,
        Some("verified") => Stage::Verified,
        _ => Stage::Investigate,
    };
    let status = match get(&fields, "status") {
        Some("superseded") => Status::Superseded,
        _ => Status::Active,
    };
    Note {
        path: path.to_path_buf(),
        name: get(&fields, "name").unwrap_or(&stem).to_string(),
        description: get(&fields, "description").unwrap_or("").to_string(),
        kind: get(&fields, "type").unwrap_or("other").to_string(),
        stage,
        status,
        verified_on: get(&fields, "verified_on").and_then(Day::parse),
        mtime,
        file,
    }
}

pub fn load_dir(dir: &Path) -> Res<Vec<Note>> {
    let mut notes = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry?.path();
        let is_note = path.extension().is_some_and(|e| e == "md")
            && path.file_name().is_some_and(|f| f != INDEX_FILE);
        if !is_note {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let mtime = std::fs::metadata(&path)?.modified()?;
        notes.push(parse(&path, &text, mtime));
    }
    notes.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(notes)
}

fn indent_of(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

fn key_of(line: &str) -> Option<&str> {
    line.trim().split_once(':').map(|(k, _)| k.trim())
}

/// Sets frontmatter fields. A field is replaced where it already is, at the top level or under
/// `metadata:`. A new field goes under `metadata:` when the note has that block, the way Claude
/// Code writes notes, and at the top level otherwise. Every other line is kept as it is.
pub fn set_fields(text: &str, updates: &[(&str, String)]) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let close = lines
        .iter()
        .skip(1)
        .position(|l| l.trim_end() == "---")
        .map(|i| i + 1)
        .filter(|_| lines.first().map(|l| l.trim_end()) == Some("---"));
    let (mut head, body): (Vec<String>, &[&str]) = match close {
        Some(c) => (
            lines[1..c].iter().map(|l| l.to_string()).collect(),
            &lines[c + 1..],
        ),
        None => (Vec::new(), &lines[..]),
    };
    for (k, v) in updates {
        let meta = head.iter().position(|l| l.trim_end() == "metadata:");
        let children = meta.map(|m| {
            let n = head[m + 1..]
                .iter()
                .take_while(|l| !indent_of(l).is_empty())
                .count();
            (m + 1, m + 1 + n)
        });
        let child_indent = children.filter(|(a, b)| a < b).map_or_else(
            || "  ".to_string(),
            |(a, _)| indent_of(&head[a]).to_string(),
        );
        let top = head
            .iter()
            .position(|l| indent_of(l).is_empty() && key_of(l) == Some(k));
        let child = children.and_then(|(a, b)| {
            (a..b).find(|&i| indent_of(&head[i]) == child_indent && key_of(&head[i]) == Some(k))
        });
        match (top, child, children) {
            (Some(i), _, _) => head[i] = format!("{k}: {v}"),
            (None, Some(i), _) => head[i] = format!("{child_indent}{k}: {v}"),
            (None, None, Some((_, end))) => head.insert(end, format!("{child_indent}{k}: {v}")),
            (None, None, None) => head.push(format!("{k}: {v}")),
        }
    }
    let mut out = vec!["---".to_string()];
    out.extend(head);
    out.push("---".into());
    out.extend(body.iter().map(|l| l.to_string()));
    let mut joined = out.join("\n");
    if text.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEW: &str = "---\nname: short-answers\ndescription: Keep replies brief\ntype: feedback\nstage: rule\nstatus: active\nverified_on: 2026-11-01\ntags: [style]\n---\nBody\n";
    const OLD: &str = "---\nname: old-note\ndescription: \"From before almanac\"\nmetadata:\n  type: project\n---\nBody\n";

    #[test]
    fn parses_almanac_notes() {
        let n = parse(
            Path::new("/v/p/short-answers.md"),
            NEW,
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(n.stage, Stage::Rule);
        assert_eq!(n.status, Status::Active);
        assert_eq!(n.kind, "feedback");
        assert_eq!(
            n.verified_on.map(Day::format).as_deref(),
            Some("2026-11-01")
        );
    }

    #[test]
    fn old_notes_default_to_investigate() {
        let n = parse(Path::new("/v/p/old-note.md"), OLD, SystemTime::UNIX_EPOCH);
        assert_eq!(n.stage, Stage::Investigate);
        assert_eq!(n.kind, "project");
        assert_eq!(n.description, "From before almanac");
    }

    #[test]
    fn set_fields_replaces_and_appends() {
        let out = set_fields(
            OLD,
            &[("name", "renamed".into()), ("stage", "verified".into())],
        );
        assert_eq!(
            out,
            "---\nname: renamed\ndescription: \"From before almanac\"\nmetadata:\n  type: project\n  stage: verified\n---\nBody\n"
        );
    }

    #[test]
    fn reads_and_writes_fields_nested_by_claude_code() {
        let text = "---\nname: n\nmetadata:\n  node_type: memory\n  type: project\n  stage: investigate\n  tags:\n    - demo\n---\nBody\n";
        let n = parse(Path::new("/v/p/n.md"), text, SystemTime::UNIX_EPOCH);
        assert_eq!((n.kind.as_str(), n.stage), ("project", Stage::Investigate));
        let out = set_fields(
            text,
            &[
                ("stage", "rule".into()),
                ("verified_on", "2026-11-10".into()),
            ],
        );
        assert_eq!(
            out,
            "---\nname: n\nmetadata:\n  node_type: memory\n  type: project\n  stage: rule\n  tags:\n    - demo\n  verified_on: 2026-11-10\n---\nBody\n"
        );
        let n = parse(Path::new("/v/p/n.md"), &out, SystemTime::UNIX_EPOCH);
        assert_eq!(n.stage, Stage::Rule);
    }

    #[test]
    fn set_fields_adds_frontmatter_when_missing() {
        assert_eq!(
            set_fields("Body\n", &[("stage", "investigate".into())]),
            "---\nstage: investigate\n---\nBody\n"
        );
    }
}

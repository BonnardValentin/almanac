use crate::decide::{self, Gate};
use crate::jev::{self, Client, Kind, Question};
use crate::{hook_input, log, str_field, Res};
use regex::Regex;
use std::sync::OnceLock;

struct Rule {
    pattern: &'static str,
    decision: Gate,
    reason: &'static str,
}

// Matched before the classifier sees anything: a command can be written to mislead a model,
// not a regex. First match wins, so the deny rules come first.
const RULES: &[Rule] = &[
    Rule {
        pattern: r"\bgit\b[^;&|]*\bpush\b[^;&|]*(\s--force(-with-lease)?\b|\s-[a-zA-Z]*f\b|\s\+\S)",
        decision: Gate::Deny,
        reason: "force push rewrites shared history",
    },
    Rule {
        pattern: r"\|\s*(sudo\s+)?(ba|z|da|k)?sh\b",
        decision: Gate::Deny,
        reason: "piping into a shell runs code nobody has read",
    },
    Rule {
        pattern: r"\brm\b[^;&|]*\s(-[a-zA-Z]*[rR][a-zA-Z]*|--recursive)\b",
        decision: Gate::Ask,
        reason: "recursive delete",
    },
    Rule {
        pattern: r"\bgit\b[^;&|]*\b(reset\b[^;&|]*--hard|clean\b[^;&|]*\s-[a-zA-Z]*f)",
        decision: Gate::Ask,
        reason: "discards uncommitted work",
    },
    Rule {
        pattern: r"\bgit\b[^;&|]*\bpush\b",
        decision: Gate::Ask,
        reason: "push publishes to a remote",
    },
];

const CLASSES: &[&str] = &[
    "read_only",
    "local_edit",
    "destructive",
    "external",
    "other",
];

const QUESTIONS: &[Question] = &[Question {
    key: "command_class",
    kind: Kind::Choice(CLASSES),
    instructions: "Classify the shell command by what it does when it runs in the working directory. \
read_only: only reads files, state or output, and changes nothing. \
local_edit: changes files inside the working directory only, and the change can be undone with git. \
destructive: deletes data, rewrites git history, or changes files outside the working directory. \
external: sends data off the machine, deploys, publishes, installs software, or spends money. \
other: none of these, or you cannot tell.",
}];

fn rules() -> &'static [(Regex, &'static Rule)] {
    static COMPILED: OnceLock<Vec<(Regex, &'static Rule)>> = OnceLock::new();
    COMPILED.get_or_init(|| {
        RULES
            .iter()
            .map(|r| {
                (
                    Regex::new(r.pattern).expect("hard rule patterns are valid"),
                    r,
                )
            })
            .collect()
    })
}

pub fn hard_rule(command: &str) -> Option<(Gate, &'static str)> {
    rules()
        .iter()
        .find(|(re, _)| re.is_match(command))
        .map(|(_, r)| (r.decision, r.reason))
}

fn emit(decision: Gate, reason: &str) -> Res {
    let out = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": decision.as_str(),
            "permissionDecisionReason": format!("almanac: {reason}"),
        }
    });
    println!("{out}");
    Ok(())
}

/// None when the gate is off, else whether classifier decisions are enforced.
fn enforce_mode() -> Res<Option<bool>> {
    let mode = std::env::var("CLAUDE_PLUGIN_OPTION_GATE_MODE").unwrap_or_default();
    match mode.as_str() {
        "off" => Ok(None),
        "shadow" | "" => Ok(Some(false)),
        "enforce" => Ok(Some(true)),
        other => Err(format!("unknown gate_mode `{other}`").into()),
    }
}

pub fn run() -> Res {
    let Some(enforce) = enforce_mode()? else {
        return Ok(());
    };
    let input = hook_input()?;
    if input["tool_name"] != "Bash" {
        return Ok(());
    }
    let command = input["tool_input"]["command"].as_str().unwrap_or_default();
    let id = input["tool_use_id"].clone();

    // Hard rules apply in shadow mode too: shadow exists to tune the classifier, not them.
    if let Some((decision, reason)) = hard_rule(command) {
        log::append(serde_json::json!({
            "hook": "gate", "by": "rule", "would": decision.as_str(), "reason": reason,
            "applied": true, "tool_use_id": id, "command": command,
        }))?;
        return emit(decision, reason);
    }
    let Some(client) = Client::from_env() else {
        return Ok(());
    };
    let state = format!(
        "Command: {command}\nWorking directory: {}",
        str_field(&input, "cwd")?
    );
    let answers = client.ask(&state, QUESTIONS)?;
    let (class, p) = jev::choice(&answers, "command_class")?;
    let decision = decide::gate(class, p);
    log::append(serde_json::json!({
        "hook": "gate", "by": "classifier", "would": decision.as_str(), "class": class, "p": p,
        "applied": enforce, "tool_use_id": id, "command": command,
    }))?;
    if enforce {
        return emit(decision, &format!("classified as {class} (p={p:.2})"));
    }
    Ok(())
}

pub fn record_ran() -> Res {
    if enforce_mode()?.is_none() {
        return Ok(());
    }
    let input = hook_input()?;
    log::append(serde_json::json!({ "hook": "ran", "tool_use_id": input["tool_use_id"] }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// test/fixtures/gate.jsonl is the gate's public eval: one command per line with the
    /// decision the hard rules must reach, or null when they must leave it to the classifier.
    #[test]
    fn hard_rules_match_the_fixtures() {
        let fixtures = include_str!("../test/fixtures/gate.jsonl");
        let mut failures = Vec::new();
        for line in fixtures.lines().filter(|l| !l.trim().is_empty()) {
            let case: serde_json::Value =
                serde_json::from_str(line).expect("fixture lines are JSON");
            let command = case["command"].as_str().expect("fixture has a command");
            let got = hard_rule(command).map(|(d, _)| d.as_str());
            if got != case["expect"].as_str() {
                failures.push(format!(
                    "{command:?}: expected {}, got {got:?}",
                    case["expect"]
                ));
            }
        }
        assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    }
}

use crate::Res;
use std::collections::HashMap;
use std::path::PathBuf;

pub enum Kind {
    /// Probability that the answer is yes.
    Noul,
    Choice(&'static [&'static str]),
}

pub struct Question {
    pub key: &'static str,
    pub kind: Kind,
    // The model never sees `key`, so the meaning has to live here. The live client sends it;
    // replay does not need it.
    #[allow(dead_code)]
    pub instructions: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    Prob(f64),
    Choice { option: String, p: f64 },
}

/// The only way almanac talks to the decision model. Every question for one decision
/// goes in a single `ask`, so the state is sent once.
pub enum Client {
    /// Answers read from a JSON file, keyed by question: `{"key": {"p": 0.9}}` or
    /// `{"key": {"option": "read_only", "p": 0.97}}`. Used by tests and offline runs.
    Replay(PathBuf),
}

impl Client {
    pub fn from_env() -> Option<Client> {
        if let Some(path) = std::env::var_os("ALMANAC_JEV_REPLAY").filter(|p| !p.is_empty()) {
            return Some(Client::Replay(PathBuf::from(path)));
        }
        let has_key = std::env::var_os("CLAUDE_PLUGIN_OPTION_TYPESAFE_API_KEY")
            .is_some_and(|k| !k.is_empty());
        if has_key {
            eprintln!("almanac: a TypeSafe key is set, but this version has no live client yet: using hard rules only");
        }
        None
    }

    pub fn ask(&self, _state: &str, questions: &[Question]) -> Res<HashMap<&'static str, Answer>> {
        match self {
            Client::Replay(path) => {
                let all: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
                questions
                    .iter()
                    .map(|q| Ok((q.key, replayed(&all[q.key], q)?)))
                    .collect()
            }
        }
    }
}

fn replayed(v: &serde_json::Value, q: &Question) -> Res<Answer> {
    let p = v["p"]
        .as_f64()
        .filter(|p| (0.0..=1.0).contains(p))
        .ok_or_else(|| format!("replay has no probability for `{}`", q.key))?;
    match q.kind {
        Kind::Noul => Ok(Answer::Prob(p)),
        Kind::Choice(options) => {
            let option = v["option"].as_str().unwrap_or_default();
            if !options.contains(&option) {
                return Err(format!(
                    "replay option `{option}` is not one of {options:?} for `{}`",
                    q.key
                )
                .into());
            }
            Ok(Answer::Choice {
                option: option.to_string(),
                p,
            })
        }
    }
}

pub fn prob(answers: &HashMap<&str, Answer>, key: &str) -> Res<f64> {
    match answers.get(key) {
        Some(Answer::Prob(p)) => Ok(*p),
        other => Err(format!("expected a probability for `{key}`, got {other:?}").into()),
    }
}

pub fn choice<'a>(answers: &'a HashMap<&str, Answer>, key: &str) -> Res<(&'a str, f64)> {
    match answers.get(key) {
        Some(Answer::Choice { option, p }) => Ok((option.as_str(), *p)),
        other => Err(format!("expected a choice for `{key}`, got {other:?}").into()),
    }
}

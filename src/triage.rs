use crate::decide;
use crate::jev::{self, Client, Kind, Question};
use crate::Res;
use std::io::BufRead;

const QUESTIONS: &[Question] = &[Question {
    key: "needs_work",
    kind: Kind::Noul,
    instructions: "The item arrived since the last check: an issue, a message or a CI result. \
Answer yes if it asks for a change, reports a failure, or asks a question that needs an answer. \
Answer no if it is a notification, a duplicate, or needs no action.",
}];

/// Reads one JSON item per line ({"id": ..., "text": ...}) and prints the id of each item that
/// needs an agent turn. The caller starts a turn only for those.
pub fn run() -> Res {
    let client = Client::from_env();
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let item: serde_json::Value = serde_json::from_str(&line)?;
        let id = item["id"].to_string();
        let keep = match &client {
            None => true,
            Some(c) => {
                let answers = c.ask(item["text"].as_str().unwrap_or_default(), QUESTIONS)?;
                decide::needs_work(jev::prob(&answers, "needs_work")?)
            }
        };
        if keep {
            println!("{}", id.trim_matches('"'));
        }
    }
    Ok(())
}

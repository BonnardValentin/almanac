use crate::paths::data_dir;
use crate::Res;
use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

const LOG_FILE: &str = "decisions.jsonl";

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// One line per decision, shadow or enforced. `shadow report` and threshold tuning read only this.
pub fn append(mut entry: serde_json::Value) -> Res {
    entry["ts"] = now_secs().into();
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(data_dir()?.join(LOG_FILE))?;
    writeln!(f, "{entry}")?;
    Ok(())
}

/// For each gate verdict, how many of those commands went on to run. A `deny` that ran, or an
/// `allow` that did not, is a place where the gate and the human disagreed.
pub fn report() -> Res {
    let text = std::fs::read_to_string(data_dir()?.join(LOG_FILE)).unwrap_or_default();
    let entries: Vec<serde_json::Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let ran: HashSet<&str> = entries
        .iter()
        .filter(|e| e["hook"] == "ran")
        .filter_map(|e| e["tool_use_id"].as_str())
        .collect();
    let mut rows: BTreeMap<(String, String), (u32, u32)> = BTreeMap::new();
    for e in entries.iter().filter(|e| e["hook"] == "gate") {
        let key = (
            e["by"].as_str().unwrap_or("?").to_string(),
            e["would"].as_str().unwrap_or("?").to_string(),
        );
        let row = rows.entry(key).or_default();
        row.0 += 1;
        if e["tool_use_id"].as_str().is_some_and(|id| ran.contains(id)) {
            row.1 += 1;
        }
    }
    if rows.is_empty() {
        println!("no gate decisions logged yet");
        return Ok(());
    }
    println!("{:<12} {:<8} {:>8} {:>8}", "by", "would", "count", "ran");
    for ((by, would), (count, ran)) in rows {
        println!("{by:<12} {would:<8} {count:>8} {ran:>8}");
    }
    Ok(())
}

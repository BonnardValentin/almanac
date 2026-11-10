mod date;
mod decide;
mod gate;
mod jev;
mod log;
mod mem;
mod notes;
mod paths;
mod session;
mod stop;
mod triage;

use std::process::ExitCode;

pub type Res<T = ()> = Result<T, Box<dyn std::error::Error>>;

const USAGE: &str = "usage:
  almanac hook session-start | stop | gate | ran     (called by the plugin's hooks)
  almanac mem index | queue | stale [--dir D] [--json]
  almanac mem apply [--dir D] < sweep-result.json
  almanac setup --vault V [--cwd C]
  almanac shadow report
  almanac triage < items.jsonl";

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
    let result = match argv.as_slice() {
        ["hook", "session-start"] => session::start(),
        ["hook", "stop"] => stop::run(),
        ["hook", "gate"] => gate::run(),
        ["hook", "ran"] => gate::record_ran(),
        ["mem", "index", rest @ ..] => mem::index(rest),
        ["mem", "queue", rest @ ..] => mem::queue(rest),
        ["mem", "stale", rest @ ..] => mem::stale(rest),
        ["mem", "apply", rest @ ..] => mem::apply(rest),
        ["setup", rest @ ..] => paths::setup(rest),
        ["shadow", "report"] => log::report(),
        ["triage"] => triage::run(),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("almanac: {e}");
            ExitCode::FAILURE
        }
    }
}

pub fn flag<'a>(args: &[&'a str], name: &str) -> Option<&'a str> {
    args.windows(2).find(|w| w[0] == name).map(|w| w[1])
}

pub fn has(args: &[&str], name: &str) -> bool {
    args.contains(&name)
}

pub fn hook_input() -> Res<serde_json::Value> {
    Ok(serde_json::from_reader(std::io::stdin().lock())?)
}

pub fn str_field<'a>(v: &'a serde_json::Value, key: &str) -> Res<&'a str> {
    v[key]
        .as_str()
        .ok_or_else(|| format!("hook input has no string field `{key}`").into())
}

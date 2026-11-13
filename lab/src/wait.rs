use crate::Result;
use crate::api::Api;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_secs(45 * 60);
const GOOD: [&str; 3] = ["success", "skipped", "neutral"];

fn poll<T>(what: &str, mut step: impl FnMut() -> Result<Option<T>>) -> Result<T> {
    let start = Instant::now();
    loop {
        if let Some(value) = step()? {
            return Ok(value);
        }
        if start.elapsed() > LIMIT {
            return Err(format!("timed out waiting for {what}").into());
        }
        std::thread::sleep(Duration::from_secs(10));
    }
}

fn list(api: &Api, path: &str, key: &str) -> Result<Vec<Value>> {
    let value = api.get(path)?.unwrap_or(Value::Null);
    Ok(value[key].as_array().cloned().unwrap_or_default())
}

fn check_state(api: &Api, sha: &str) -> Result<Option<()>> {
    let checks = list(
        api,
        &format!("commits/{sha}/check-runs?per_page=100"),
        "check_runs",
    )?;
    let runs = list(
        api,
        &format!("actions/runs?head_sha={sha}&per_page=100"),
        "workflow_runs",
    )?;
    let status = api
        .get(&format!("commits/{sha}/status"))?
        .unwrap_or(Value::Null);
    let state = status["state"].as_str().unwrap_or("");
    let statuses = status["total_count"].as_u64().unwrap_or(0);
    let all: Vec<&Value> = checks.iter().chain(runs.iter()).collect();
    let bad = |item: &&Value| {
        item["status"] == "completed" && !GOOD.contains(&item["conclusion"].as_str().unwrap_or(""))
    };
    if all.iter().any(bad) || state == "failure" || state == "error" {
        return Err(format!("checks failed on {sha}").into());
    }
    let waiting = all.iter().any(|item| item["status"] != "completed")
        || (state == "pending" && statuses > 0);
    let seen = !all.is_empty() || statuses > 0;
    Ok((seen && !waiting).then_some(()))
}

pub fn green(api: &Api, sha: &str) -> Result<()> {
    poll(&format!("checks of {sha}"), || check_state(api, sha))
}

pub fn merge(api: &Api, number: u64) -> Result<String> {
    let pull = api
        .get(&format!("pulls/{number}"))?
        .ok_or("pull request not found")?;
    green(api, pull["head"]["sha"].as_str().ok_or("no head sha")?)?;
    let merged = api.send(
        "PUT",
        &format!("pulls/{number}/merge"),
        &json!({"merge_method": "squash"}),
    )?;
    Ok(merged["sha"].as_str().ok_or("no merge sha")?.to_owned())
}

pub fn run_for(api: &Api, workflow: &str, event: &str, sha: &str) -> Result<u64> {
    let path = format!("actions/workflows/{workflow}/runs?event={event}&head_sha={sha}");
    poll(&format!("{workflow} {event} run for {sha}"), || {
        Ok(list(api, &path, "workflow_runs")?
            .first()
            .and_then(|run| run["id"].as_u64()))
    })
}

pub fn finished(api: &Api, run_id: u64) -> Result<()> {
    poll(&format!("run {run_id}"), || {
        let run = api
            .get(&format!("actions/runs/{run_id}"))?
            .unwrap_or(Value::Null);
        Ok((run["status"] == "completed").then_some(()))
    })
}

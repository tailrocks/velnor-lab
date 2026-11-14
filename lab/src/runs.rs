use crate::api::Api;
use crate::branch::{Opened, retire};
use crate::scenario::{self, Scenario};
use crate::{Result, matrix, wait};
use serde_json::{Value, json};

fn record(api: &Api, workflow: &str, run_id: u64, file: &str) -> Result<String> {
    let conclusion = wait::finished(api, run_id)?;
    let run = api
        .get(&format!("actions/runs/{run_id}"))?
        .ok_or("run not found")?;
    let jobs = api
        .get(&format!("actions/runs/{run_id}/jobs?per_page=100"))?
        .unwrap_or(Value::Null);
    let sha = run["head_sha"].as_str().unwrap_or_default();
    let on_sha = api
        .get(&format!("actions/runs?head_sha={sha}&per_page=100"))?
        .unwrap_or(Value::Null);
    let runs_on_sha: Vec<Value> = on_sha["workflow_runs"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|other| json!({"id": other["id"], "name": other["name"], "event": other["event"]}))
        .collect();
    let mut log = String::new();
    for job in jobs["jobs"].as_array().cloned().unwrap_or_default() {
        let text = api
            .download(&format!("actions/jobs/{}/logs", job["id"]))
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_else(|error| format!("log not available: {error}\n"));
        log.push_str(&format!("===== job {} {}\n{text}", job["id"], job["name"]));
    }
    let document = json!({"run": run, "jobs": jobs, "runs_on_sha": runs_on_sha});
    std::fs::write(
        format!("{file}.api.json"),
        serde_json::to_string_pretty(&document)?,
    )?;
    std::fs::write(format!("{file}.log"), log)?;
    if conclusion == "success" {
        matrix::collect(api, workflow, run_id, &format!("{file}.json"))?;
    }
    println!("{file} run {run_id} {conclusion}");
    Ok(conclusion)
}

fn pull(api: &Api, workflow: &str, scenario: Scenario, file: &str) -> Result<Opened> {
    let opened = scenario::open(api, scenario)?;
    let recorded = wait::run_for(api, workflow, "pull_request", &opened.sha)
        .and_then(|run_id| record(api, workflow, run_id, file));
    match recorded {
        Ok(_) => Ok(opened),
        Err(error) => {
            let _ = retire(api, &opened);
            Err(error)
        }
    }
}

fn once(api: &Api, workflow: &str, name: &str, file: &str) -> Result<Option<Opened>> {
    match name {
        "cold" => {
            let sha = matrix::cold(api)?;
            let run_id = wait::run_for(api, workflow, "push", &sha)?;
            record(api, workflow, run_id, file)?;
            Ok(None)
        }
        "leaf2" => {
            let mut opened = pull(api, workflow, Scenario::Leaf, &format!("{file}-push1"))?;
            let recorded = scenario::push(api, &mut opened, Scenario::Leaf)
                .and_then(|sha| wait::run_for(api, workflow, "pull_request", &sha))
                .and_then(|id| record(api, workflow, id, &format!("{file}-push2")));
            match recorded {
                Ok(_) => Ok(Some(opened)),
                Err(error) => {
                    let _ = retire(api, &opened);
                    Err(error)
                }
            }
        }
        _ => {
            let scenario = Scenario::parse(name).ok_or("unknown scenario")?;
            pull(api, workflow, scenario, file).map(Some)
        }
    }
}

pub fn run(api: &Api, dir: &str, workflow: &str, name: &str, count: usize) -> Result<()> {
    let dir = format!("{dir}/{workflow}");
    std::fs::create_dir_all(&dir)?;
    for index in 1..=count {
        let file = format!("{dir}/{name}-{index}");
        if let Some(opened) = once(api, workflow, name, &file)? {
            retire(api, &opened)?;
        }
    }
    Ok(())
}

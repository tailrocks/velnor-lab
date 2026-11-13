use crate::api::Api;
use crate::branch::{Opened, retire};
use crate::scenario::{self, Scenario};
use crate::{Result, wait};
use std::process::Command;

const PLAN: [&str; 10] = [
    "cold", "warm", "warm", "warm", "leaf", "leaf", "leaf", "hub", "lock", "docs",
];

fn delete_caches(api: &Api) -> Result<()> {
    loop {
        let page = api.get("actions/caches?per_page=100")?.unwrap_or_default();
        let caches = page["actions_caches"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if caches.is_empty() {
            return Ok(());
        }
        for cache in caches {
            api.delete(&format!("actions/caches/{}", cache["id"]))?;
        }
    }
}

pub fn cold(api: &Api) -> Result<String> {
    delete_caches(api)?;
    let opened = scenario::open(api, Scenario::Warm)?;
    let merged = wait::merge(api, opened.number);
    retire(api, &opened)?;
    merged
}

fn collect(api: &Api, workflow: &str, run_id: u64, file: &str) -> Result<()> {
    let program = std::env::var("VELNOR_CI").unwrap_or_else(|_| "velnor-ci".into());
    let status = Command::new(program)
        .args([
            "bench",
            "collect",
            "--repo",
            &api.repo,
            "-o",
            file,
            &run_id.to_string(),
        ])
        .env("GITHUB_TOKEN", api.token())
        .status()?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("collect failed for {workflow} run {run_id}").into())
}

fn step(api: &Api, workflow: &str, name: &str, file: &str, opened: &mut Vec<Opened>) -> Result<()> {
    let run_id = if name == "cold" {
        let sha = cold(api)?;
        wait::run_for(api, workflow, "push", &sha)?
    } else {
        let scenario = Scenario::parse(name).ok_or("unknown scenario")?;
        let pull = scenario::open(api, scenario)?;
        let run_id = wait::run_for(api, workflow, "pull_request", &pull.sha);
        opened.push(pull);
        run_id?
    };
    wait::finished(api, run_id)?;
    collect(api, workflow, run_id, file)
}

pub fn run(api: &Api, workflow: &str) -> Result<()> {
    let dir = format!("results/{workflow}");
    std::fs::create_dir_all(&dir)?;
    let mut opened = Vec::new();
    let mut outcome = Ok(());
    for (index, name) in PLAN.iter().enumerate() {
        let file = format!("{dir}/{index:02}-{name}.json");
        outcome = step(api, workflow, name, &file, &mut opened);
        if outcome.is_err() {
            break;
        }
    }
    for pull in &opened {
        retire(api, pull)?;
    }
    outcome
}

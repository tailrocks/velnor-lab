use crate::api::Api;
use crate::branch::{Opened, retire, retire_branch};
use crate::{Result, signer, wait};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Map, Value, json};
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

pub const NAMES: [&str; 8] = [
    "token",
    "cache-save-existing",
    "cache-restore-keys",
    "cache-pr-reads",
    "cache-other-pr",
    "cache-own-pr",
    "needs-result",
    "event-payload",
];

struct Session<'a> {
    api: &'a Api,
    base: String,
    tip: String,
    tag: String,
    signer: (String, String),
    tracked: Vec<Opened>,
}

impl Session<'_> {
    fn track(&mut self, opened: Opened) {
        self.tracked.retain(|item| item.branch != opened.branch);
        self.tracked.push(opened);
    }

    fn branch(&mut self, role: &str, from: &str) -> Result<String> {
        let name = format!("probe-{}-{role}", self.tag);
        let body = json!({"ref": format!("refs/heads/{name}"), "sha": from});
        self.api.send("POST", "git/refs", &body)?;
        self.track(Opened {
            number: 0,
            url: String::new(),
            branch: name.clone(),
            sha: from.to_owned(),
        });
        Ok(name)
    }

    fn commit(&mut self, probe: &str, branch: &str, add_line: bool) -> Result<String> {
        let path = format!("probes/{probe}/runs/{branch}.txt");
        let file = self.api.get(&format!("contents/{path}?ref={branch}"))?;
        let mut content = match &file {
            Some(file) => String::from_utf8(
                STANDARD.decode(file["content"].as_str().unwrap_or("").replace('\n', ""))?,
            )?,
            None => String::new(),
        };
        content.push_str(&format!("{}\n", if add_line { "next" } else { "first" }));
        let (name, email) = &self.signer;
        let who = json!({"name": name, "email": email});
        let mut body = json!({
            "message": format!("Probe {probe} {}\n\nSigned-off-by: {name} <{email}>", self.tag),
            "content": STANDARD.encode(content),
            "branch": branch,
            "author": who,
            "committer": who,
        });
        if let Some(file) = &file {
            body["sha"] = file["sha"].clone();
        }
        let done = self.api.send("PUT", &format!("contents/{path}"), &body)?;
        let sha = done["commit"]["sha"]
            .as_str()
            .ok_or("no commit sha")?
            .to_owned();
        if let Some(item) = self.tracked.iter_mut().find(|item| item.branch == branch) {
            item.sha.clone_from(&sha);
        }
        Ok(sha)
    }

    fn pull(&mut self, probe: &str, branch: &str, base: &str) -> Result<Opened> {
        let title = format!("Probe {probe} {}", self.tag);
        let body = json!({"title": title, "head": branch, "base": base, "body": "Opened by `lab probes`."});
        let pull = self.api.send("POST", "pulls", &body)?;
        let sha = self
            .tracked
            .iter()
            .find(|item| item.branch == branch)
            .map(|item| item.sha.clone())
            .ok_or("branch is not tracked")?;
        let opened = Opened {
            number: pull["number"].as_u64().ok_or("no pull number")?,
            url: pull["html_url"].as_str().unwrap_or_default().to_owned(),
            branch: branch.to_owned(),
            sha,
        };
        self.tracked.retain(|item| item.branch != branch);
        self.tracked.push(Opened {
            number: opened.number,
            url: opened.url.clone(),
            branch: opened.branch.clone(),
            sha: opened.sha.clone(),
        });
        Ok(opened)
    }

    fn open_pull(&mut self, probe: &str, role: &str, base: &str, from: &str) -> Result<Opened> {
        let branch = self.branch(role, from)?;
        self.commit(probe, &branch, false)?;
        self.pull(probe, &branch, base)
    }

    fn dispatch(&self, probe: &str, git_ref: &str, role: &str) -> Result<u64> {
        let path = format!("actions/workflows/probe-{probe}.yml");
        let list = format!("{path}/runs?event=workflow_dispatch&branch={git_ref}&per_page=100");
        let known = run_ids(self.api, &list)?;
        let inputs = json!({"ref": git_ref, "inputs": {"tag": self.tag, "role": role}});
        self.api
            .send("POST", &format!("{path}/dispatches"), &inputs)?;
        wait::poll(&format!("dispatched run of {probe}"), || {
            Ok(run_ids(self.api, &list)?
                .into_iter()
                .find(|id| !known.contains(id)))
        })
    }

    fn finished(&self, run: u64, expected: &str) -> Result<u64> {
        let conclusion = wait::finished(self.api, run)?;
        if conclusion == expected {
            return Ok(run);
        }
        Err(format!("run {run} concluded {conclusion}, expected {expected}").into())
    }

    fn runs(&mut self, probe: &str, index: usize) -> Result<Vec<u64>> {
        let (api, base, tip) = (self.api, self.base.clone(), self.tip.clone());
        let file = format!("probe-{probe}.yml");
        match probe {
            "token" | "event-payload" => {
                let mut runs = Vec::new();
                if probe == "event-payload" {
                    runs.push(self.dispatch(probe, &base, "manual")?);
                }
                let pull = self.open_pull(probe, "events", &base, &tip)?;
                let push = wait::run_for(api, &file, "push", &pull.sha)?;
                let request = wait::run_for(api, &file, "pull_request", &pull.sha)?;
                runs.extend([push, request]);
                runs.extend(latest_schedule(api, &file)?);
                Ok(runs)
            }
            "cache-save-existing" | "cache-restore-keys" => {
                Ok(vec![self.dispatch(probe, &base, "manual")?])
            }
            "needs-result" => Ok(vec![self.dispatch(probe, &base, "manual")?]),
            "cache-pr-reads" => {
                let base_branch = self.branch("base", &tip)?;
                let head = self.branch("head", &tip)?;
                self.commit(probe, &head, false)?;
                for (git_ref, role) in [(&base, "default"), (&base_branch, "base"), (&head, "head")]
                {
                    let run = self.dispatch(probe, git_ref, role)?;
                    self.finished(run, "success")?;
                }
                let pull = self.pull(probe, &head, &base_branch)?;
                Ok(vec![wait::run_for(api, &file, "pull_request", &pull.sha)?])
            }
            "cache-other-pr" => {
                let first = self.open_pull(probe, "a", &base, &tip)?;
                let run = wait::run_for(api, &file, "pull_request", &first.sha)?;
                self.finished(run, "success")?;
                let second = self.open_pull(probe, "b", &base, &tip)?;
                let other = wait::run_for(api, &file, "pull_request", &second.sha)?;
                Ok(vec![run, other])
            }
            "cache-own-pr" => {
                let pull = self.open_pull(probe, "own", &base, &tip)?;
                let first = wait::run_for(api, &file, "pull_request", &pull.sha)?;
                self.finished(first, "success")?;
                let sha = self.commit(probe, &pull.branch, true)?;
                let second = wait::run_for(api, &file, "pull_request", &sha)?;
                Ok(vec![first, second])
            }
            _ => Err(format!("unknown probe {probe} ({index})").into()),
        }
    }
}

fn run_ids(api: &Api, path: &str) -> Result<Vec<u64>> {
    let page = api.get(path)?.unwrap_or(Value::Null);
    Ok(page["workflow_runs"]
        .as_array()
        .map(|runs| runs.iter().filter_map(|run| run["id"].as_u64()).collect())
        .unwrap_or_default())
}

fn latest_schedule(api: &Api, file: &str) -> Result<Option<u64>> {
    let path = format!("actions/workflows/{file}/runs?event=schedule&status=completed&per_page=20");
    for run in run_ids(api, &path)? {
        if artifact_url(api, run)?.is_some() {
            return Ok(Some(run));
        }
    }
    Ok(None)
}

fn artifact_url(api: &Api, run: u64) -> Result<Option<String>> {
    let list = api
        .get(&format!("actions/runs/{run}/artifacts"))?
        .unwrap_or(Value::Null);
    Ok(list["artifacts"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["name"] == "observation"))
        .and_then(|item| item["archive_download_url"].as_str())
        .map(str::to_owned))
}

fn documents(api: &Api, run: u64) -> Result<Vec<Value>> {
    let mut attempts = 0;
    let url = wait::poll(&format!("the observation of run {run}"), || {
        attempts += 1;
        let url = artifact_url(api, run)?;
        if url.is_none() && attempts > 6 {
            return Err(format!("run {run} has no observation artifact").into());
        }
        Ok(url)
    })?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(api.download(&url)?))?;
    let mut found = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let mut text = String::new();
        file.read_to_string(&mut text)?;
        found.push(serde_json::from_str(&text)?);
    }
    Ok(found)
}

pub fn merge_legs(into: &mut Map<String, Value>, document: &Value) {
    if let Some(legs) = document["legs"].as_object() {
        for (leg, value) in legs {
            into.insert(leg.clone(), value.clone());
        }
    }
}

pub fn differing_legs(stored: &Value, fresh: &Map<String, Value>) -> Vec<String> {
    fresh
        .iter()
        .filter(|(leg, value)| stored["legs"].get(leg.as_str()) != Some(*value))
        .map(|(leg, _)| leg.clone())
        .collect()
}

fn conformance_file(probe: &str) -> String {
    let dir = std::env::var("LAB_CONFORMANCE").unwrap_or_else(|_| "conformance".into());
    format!("{dir}/{probe}.json")
}

fn stored(probe: &str) -> Value {
    std::fs::read_to_string(conformance_file(probe))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({"probe": probe, "legs": {}}))
}

fn observe(session: &mut Session<'_>, probe: &str, index: usize) -> Result<Map<String, Value>> {
    let mut legs = Map::new();
    for run in session.runs(probe, index)? {
        wait::finished(session.api, run)?;
        for document in documents(session.api, run)? {
            merge_legs(&mut legs, &document);
        }
    }
    Ok(legs)
}

fn probe(api: &Api, name: &str, index: usize, check: bool) -> Result<()> {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let base = api.base();
    let tip = api
        .get(&format!("git/ref/heads/{base}"))?
        .ok_or("base branch not found")?["object"]["sha"]
        .as_str()
        .ok_or("no base sha")?
        .to_owned();
    let mut session = Session {
        api,
        base,
        tip,
        tag: format!("{secs}{index}"),
        signer: signer::get(api)?,
        tracked: Vec::new(),
    };
    let observed = observe(&mut session, name, index);
    let mut failures = Vec::new();
    for item in session.tracked.iter().rev() {
        let done = if item.number == 0 {
            retire_branch(api, item)
        } else {
            retire(api, item)
        };
        if let Err(error) = done {
            failures.push(error.to_string());
        }
    }
    let legs = observed?;
    let current = stored(name);
    if check {
        let differing = differing_legs(&current, &legs);
        if !differing.is_empty() {
            return Err(format!(
                "probe {name} differs from {} in {}",
                conformance_file(name),
                differing.join(", ")
            )
            .into());
        }
    } else {
        let mut merged = current["legs"].as_object().cloned().unwrap_or_default();
        merged.extend(legs);
        let document = json!({"probe": name, "legs": merged});
        std::fs::create_dir_all(
            conformance_file(name)
                .rsplit_once('/')
                .map_or(".", |(dir, _)| dir),
        )?;
        std::fs::write(
            conformance_file(name),
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        )?;
        println!("wrote {}", conformance_file(name));
    }
    failures
        .is_empty()
        .then_some(())
        .ok_or_else(|| failures.join("; ").into())
}

pub fn run(api: &Api, args: &[&str]) -> Result<()> {
    let check = args.contains(&"--check");
    let chosen: Vec<&str> = args
        .iter()
        .copied()
        .filter(|arg| *arg != "--check")
        .collect();
    if let Some(unknown) = chosen.iter().find(|name| !NAMES.contains(name)) {
        return Err(format!("unknown probe {unknown}; probes: {}", NAMES.join(", ")).into());
    }
    let mut failed = Vec::new();
    for (index, name) in NAMES.iter().enumerate() {
        if !chosen.is_empty() && !chosen.contains(name) {
            continue;
        }
        if let Err(error) = probe(api, name, index, check) {
            eprintln!("lab: probe {name}: {error}");
            failed.push(*name);
        }
    }
    if failed.is_empty() {
        return Ok(());
    }
    Err(format!("probes failed: {}", failed.join(", ")).into())
}

#[cfg(test)]
mod tests {
    use super::{differing_legs, merge_legs};
    use serde_json::{Map, json};

    #[test]
    fn legs_of_several_runs_merge_into_one_document() {
        let mut legs = Map::new();
        merge_legs(
            &mut legs,
            &json!({"probe": "token", "legs": {"push": {"a": 1}}}),
        );
        merge_legs(
            &mut legs,
            &json!({"probe": "token", "legs": {"pull_request": {"a": 2}}}),
        );
        assert_eq!(legs.len(), 2);
    }

    #[test]
    fn a_leg_that_changed_or_is_missing_from_the_stored_file_differs() {
        let stored = json!({"probe": "token", "legs": {"push": {"a": 1}, "schedule": {"a": 1}}});
        let mut fresh = Map::new();
        fresh.insert("push".into(), json!({"a": 1}));
        fresh.insert("schedule".into(), json!({"a": 2}));
        fresh.insert("workflow_dispatch".into(), json!({"a": 1}));
        assert_eq!(
            differing_legs(&stored, &fresh),
            ["schedule", "workflow_dispatch"]
        );
    }
}

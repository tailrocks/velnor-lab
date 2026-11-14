mod reach;

use crate::Result;
use reach::{Named, Selected, members, missed, named, parse_explain, reach};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

const LOCKS: &str = ":(glob)**/Cargo.lock";
const MANIFESTS: &str = ":(glob)**/Cargo.toml";
const CONFIG: &str = ".velnor/rust-ci.toml";

fn capture(program: &str, args: &[&str], directory: &Path) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(directory)
        .output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let head: String = stderr.trim().chars().take(240).collect();
        Err(format!("{program} failed: {}", head.replace('\n', " ")).into())
    }
}

fn git(directory: &Path, args: &[&str]) -> Result<String> {
    capture("git", args, directory)
}

fn cargo(directory: &Path, args: &[&str]) -> Result<String> {
    let program = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    capture(&program, args, directory)
}

fn manifest_of(lock: &str) -> String {
    match lock.rsplit_once('/') {
        Some((directory, _)) => format!("{directory}/Cargo.toml"),
        None => "Cargo.toml".to_owned(),
    }
}

fn metadata(work: &Path, lock: &str) -> Result<reach::Reach> {
    let manifest = work.join(manifest_of(lock));
    let json = cargo(
        work,
        &[
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--manifest-path",
            &manifest.to_string_lossy(),
        ],
    )
    .map_err(|e| format!("cargo metadata --locked of {}: {e}", manifest_of(lock)))?;
    reach(&serde_json::from_str(&json)?)
}

fn unusable(work: &Path, manifest: &str, tracked: &BTreeSet<&str>) -> bool {
    let path = work.join(manifest).to_string_lossy().into_owned();
    let args = [
        "metadata",
        "--no-deps",
        "--offline",
        "--format-version",
        "1",
        "--manifest-path",
        &path,
    ];
    let Ok(json) = cargo(work, &args) else {
        return true;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
        return true;
    };
    let directory = manifest
        .rsplit_once('/')
        .map_or("", |(directory, _)| directory);
    let own_root = value["workspace_root"]
        .as_str()
        .is_some_and(|root| Path::new(root) == work.join(directory));
    let lock = format!("{}Cargo.lock", manifest.trim_end_matches("Cargo.toml"));
    own_root && !tracked.contains(lock.as_str())
}

fn configure(work: &Path) -> Result<usize> {
    if git(work, &["ls-files", "--error-unmatch", CONFIG]).is_ok() {
        return Ok(0);
    }
    let listed = git(work, &["ls-files", "--", MANIFESTS, LOCKS])?;
    let tracked: BTreeSet<&str> = listed.lines().collect();
    let broken: Vec<String> = tracked
        .iter()
        .filter(|path| path.ends_with("Cargo.toml") && unusable(work, path, &tracked))
        .map(|manifest| format!("\"{manifest}\""))
        .collect();
    if !broken.is_empty() {
        std::fs::create_dir_all(work.join(".velnor"))?;
        std::fs::write(
            work.join(CONFIG),
            format!("exclude = [{}]\n", broken.join(", ")),
        )?;
        git(work, &["add", "--force", CONFIG])?;
    }
    Ok(broken.len())
}

struct Outcome {
    lock: String,
    named: Named,
    selected: Selected,
    members: BTreeSet<String>,
    excluded: usize,
}

fn require(work: &Path, revision: &str, side: &str, locks: &[String]) -> Result<()> {
    for file in locks
        .iter()
        .flat_map(|lock| [lock.clone(), manifest_of(lock)])
    {
        git(work, &["cat-file", "-e", &format!("{revision}:{file}")])
            .map_err(|_| format!("{file} does not exist in {side}"))?;
    }
    Ok(())
}

fn checkout(work: &Path, revision: &str) -> Result<()> {
    git(
        work,
        &["checkout", "--quiet", "--force", "--detach", revision],
    )?;
    git(work, &["clean", "--quiet", "-fd"])?;
    Ok(())
}

fn replay_commit(work: &Path, helper: &str, scratch: &Path, sha: &str) -> Result<Vec<Outcome>> {
    let parent = git(work, &["rev-parse", &format!("{sha}^")])
        .map_err(|_| "the commit has no parent")?
        .trim()
        .to_owned();
    let diff = [
        "diff",
        "--name-only",
        "--no-renames",
        &parent,
        sha,
        "--",
        LOCKS,
    ];
    let locks: Vec<String> = git(work, &diff)?.lines().map(str::to_owned).collect();
    require(work, &parent, "the parent", &locks)?;
    require(work, sha, "the commit", &locks)?;
    checkout(work, &parent)?;
    let mut before = BTreeMap::new();
    let mut bases = Vec::new();
    for (index, lock) in locks.iter().enumerate() {
        before.insert(lock.clone(), metadata(work, lock)?);
        let file = scratch.join(format!("base-{index}"));
        std::fs::write(&file, std::fs::read_to_string(work.join(lock))?)?;
        bases.push(format!("{lock}={}", file.display()));
    }
    checkout(work, sha)?;
    let mut after = BTreeMap::new();
    for lock in &locks {
        after.insert(lock.clone(), metadata(work, lock)?);
    }
    let excluded = configure(work)?;
    let mut args = vec!["plan", "--explain", "--changed"];
    args.extend(locks.iter().map(String::as_str));
    for base in &bases {
        args.extend(["--base-file", base]);
    }
    let selections =
        parse_explain(&capture(helper, &args, work).map_err(|e| format!("velnor-ci plan: {e}"))?);
    locks
        .iter()
        .map(|lock| {
            let manifest = manifest_of(lock);
            let selected = selections
                .get(&manifest)
                .ok_or_else(|| format!("velnor-ci plan lists no root {manifest}"))?;
            Ok(Outcome {
                lock: lock.clone(),
                named: named(&before[lock], &after[lock]),
                selected: selected.clone(),
                members: members(&after[lock]),
                excluded,
            })
        })
        .collect()
}

fn describe(selected: &Selected) -> String {
    match selected {
        Selected::Nothing => "none".to_owned(),
        Selected::Some(set) => format!("some {}", set.len()),
        Selected::All(Some(set)) => format!("all (most-packages {})", set.len()),
        Selected::All(None) => "all (fallback)".to_owned(),
    }
}

#[derive(Default)]
struct Totals {
    comparisons: usize,
    named: usize,
    version_only: usize,
    missed: usize,
    by_kind: [usize; 3],
    generated_config: usize,
    unresolved: Vec<(String, String)>,
}

impl Totals {
    fn add(&mut self, sha: &str, outcome: &Outcome) {
        let lost = missed(&outcome.named.changed, &outcome.selected, &outcome.members);
        self.comparisons += 1;
        self.named += outcome.named.changed.len();
        self.version_only += outcome.named.version_only.len();
        self.missed += lost.len();
        self.generated_config += usize::from(outcome.excluded > 0);
        self.by_kind[match outcome.selected {
            Selected::Nothing | Selected::Some(_) => 0,
            Selected::All(Some(_)) => 1,
            Selected::All(None) => 2,
        }] += 1;
        println!(
            "{} {} cargo names {} of {} members, lock_affected selects {}, missed {lost:?}",
            &sha[..12],
            outcome.lock,
            outcome.named.changed.len(),
            outcome.members.len(),
            describe(&outcome.selected),
        );
    }

    fn summary(&self, root: &Path, commits: usize) -> String {
        format!(
            "replay-lock {}: {commits} Cargo.lock-changing commits, {} resolved, {} unresolved; {} lockfile comparisons, {} members named by cargo, {} members missed; selected by dependencies: {}, by dependencies but more than half the members: {}, by falling back to everything: {}; {} members whose only change is their own version; {} comparisons ran with a generated exclude list",
            root.display(),
            commits - self.unresolved.len(),
            self.unresolved.len(),
            self.comparisons,
            self.named,
            self.missed,
            self.by_kind[0],
            self.by_kind[1],
            self.by_kind[2],
            self.version_only,
            self.generated_config,
        )
    }
}

pub fn run(checkout: &str, count: usize, every_ref: bool) -> Result<()> {
    let helper = std::env::var("VELNOR_CI").unwrap_or_else(|_| "velnor-ci".to_owned());
    let root = PathBuf::from(git(Path::new(checkout), &["rev-parse", "--show-toplevel"])?.trim());
    let limit = count.to_string();
    let scope = if every_ref { "--all" } else { "HEAD" };
    let log = [
        "log",
        "-n",
        &limit,
        "--no-merges",
        "--format=%H",
        scope,
        "--",
        LOCKS,
    ];
    let shas: Vec<String> = git(&root, &log)?.lines().map(str::to_owned).collect();
    let scratch = std::env::temp_dir().join(format!("lab-replay-lock-{}", std::process::id()));
    std::fs::create_dir_all(&scratch)?;
    let work = scratch.join("work");
    let clone = [
        "clone",
        "--quiet",
        "--shared",
        "--no-checkout",
        &root.to_string_lossy(),
        &work.to_string_lossy(),
    ];
    git(&scratch, &clone)?;
    let mut totals = Totals::default();
    for sha in &shas {
        match replay_commit(&work, &helper, &scratch, sha) {
            Ok(outcomes) => outcomes.iter().for_each(|outcome| totals.add(sha, outcome)),
            Err(error) => {
                println!("{} unresolved: {error}", &sha[..12]);
                totals.unresolved.push((sha.clone(), error.to_string()));
            }
        }
    }
    std::fs::remove_dir_all(&scratch)?;
    for (sha, reason) in &totals.unresolved {
        println!("unresolved {sha}: {reason}");
    }
    println!("{}", totals.summary(&root, shas.len()));
    if totals.missed > 0 {
        return Err(format!("{} members missed", totals.missed).into());
    }
    Ok(())
}

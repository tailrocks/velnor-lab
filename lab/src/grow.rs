use crate::Result;
use std::fs;
use std::process::Command;

const SOURCE: &str = include_str!("../templates/crate.rs.txt");

fn dependencies(index: usize) -> Vec<usize> {
    match index {
        0 => vec![],
        1 => vec![0],
        _ => vec![index - 1, 0],
    }
}

fn name(index: usize) -> String {
    format!("grow-{index:03}")
}

pub fn manifest(index: usize) -> String {
    let mut text = format!(
        "[package]\nname = \"{}\"\nedition.workspace = true\nrust-version.workspace = true\nversion.workspace = true\nlicense.workspace = true\npublish.workspace = true\n\n[dependencies]\n",
        name(index)
    );
    for dep in dependencies(index) {
        text.push_str(&format!("{} = {{ path = \"../g{dep:03}\" }}\n", name(dep)));
    }
    text
}

pub fn source(index: usize) -> String {
    let calls: String = dependencies(index)
        .iter()
        .map(|dep| {
            format!(
                "\n        .wrapping_add({}::total(value))",
                name(*dep).replace('-', "_")
            )
        })
        .collect();
    SOURCE
        .replace("{ID}", &index.to_string())
        .replace("{CALLS}", &calls)
}

pub fn add_member(manifest: &str) -> Option<String> {
    let lines: Vec<&str> = manifest.lines().collect();
    let open = lines
        .iter()
        .position(|line| line.starts_with("members = ["))?;
    let close = open + lines[open..].iter().position(|line| *line == "]")?;
    let mut out: Vec<&str> = lines[..close].to_vec();
    out.push("  \"grow/*\",");
    out.extend(&lines[close..]);
    Some(out.join("\n") + "\n")
}

fn git(args: &[&str]) -> Result<()> {
    Command::new("git")
        .args(args)
        .status()?
        .success()
        .then_some(())
        .ok_or_else(|| format!("git {args:?} failed").into())
}

pub fn run(count: usize) -> Result<()> {
    git(&["switch", "-c", &format!("lab/grow-{count}")])?;
    for index in 0..count {
        let dir = format!("grow/g{index:03}");
        fs::create_dir_all(format!("{dir}/src"))?;
        fs::write(format!("{dir}/Cargo.toml"), manifest(index))?;
        fs::write(format!("{dir}/src/lib.rs"), source(index))?;
    }
    let root = fs::read_to_string("Cargo.toml")?;
    fs::write(
        "Cargo.toml",
        add_member(&root).ok_or("Cargo.toml has no members list")?,
    )?;
    Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .output()?;
    git(&["add", "-A"])?;
    git(&[
        "commit",
        "-s",
        "-m",
        &format!("Grow the workspace by {count} crates"),
    ])
}

#[cfg(test)]
mod tests;

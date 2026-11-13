use crate::api::Api;
use crate::branch::{Opened, tag};
use crate::{Result, lock, text};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::json;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    Warm,
    Docs,
    Leaf,
    Hub,
    Lock,
}

impl Scenario {
    pub fn parse(name: &str) -> Option<Self> {
        [Self::Warm, Self::Docs, Self::Leaf, Self::Hub, Self::Lock]
            .into_iter()
            .find(|scenario| scenario.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Warm => "warm",
            Self::Docs => "docs",
            Self::Leaf => "leaf",
            Self::Hub => "hub",
            Self::Lock => "lock",
        }
    }

    fn target(self) -> &'static str {
        match self {
            Self::Warm => "rust-toolchain.toml",
            Self::Docs => "docs/notes.md",
            Self::Leaf => "app/src/main.rs",
            Self::Hub => "core/src/lib.rs",
            Self::Lock => "Cargo.lock",
        }
    }

    fn edit(self, content: &str, tag: &str) -> Result<String> {
        let name = self.name();
        Ok(match self {
            Self::Warm => text::append_line(content, &format!("# lab {name} {tag}")),
            Self::Docs => text::append_line(content, &format!("- lab {name} {tag}")),
            Self::Leaf | Self::Hub => text::append_line(content, &format!("// lab {name} {tag}")),
            Self::Lock => lock::bump(content)?,
        })
    }
}

pub fn open(api: &Api, scenario: Scenario) -> Result<Opened> {
    let base = api.base();
    let tip = api
        .get(&format!("git/ref/heads/{base}"))?
        .ok_or("base branch not found")?;
    let tag = tag();
    let branch = format!("lab/{}-{tag}", scenario.name());
    let ref_body = json!({"ref": format!("refs/heads/{branch}"), "sha": tip["object"]["sha"]});
    api.send("POST", "git/refs", &ref_body)?;

    let path = scenario.target();
    let file = api.get(&format!("contents/{path}?ref={branch}"))?;
    let current = match &file {
        Some(file) => String::from_utf8(
            STANDARD.decode(file["content"].as_str().unwrap_or("").replace('\n', ""))?,
        )?,
        None => String::new(),
    };
    let (name, email) = crate::signer::get(api)?;
    let who = json!({"name": name, "email": email});
    let mut body = json!({
        "message": format!("Lab {} {tag}\n\nSigned-off-by: {name} <{email}>", scenario.name()),
        "content": STANDARD.encode(scenario.edit(&current, &tag)?),
        "branch": branch,
        "author": who,
        "committer": who,
    });
    if let Some(file) = &file {
        body["sha"] = file["sha"].clone();
    }
    let commit = api.send("PUT", &format!("contents/{path}"), &body)?;
    let sha = commit["commit"]["sha"]
        .as_str()
        .ok_or("no commit sha")?
        .to_owned();

    let title = format!("Lab {} {tag}", scenario.name());
    let pull = json!({"title": title, "head": branch, "base": base, "body": "Opened by `lab`."});
    let pull = api.send("POST", "pulls", &pull)?;
    Ok(Opened {
        number: pull["number"].as_u64().ok_or("no pull number")?,
        url: pull["html_url"].as_str().unwrap_or_default().to_owned(),
        branch,
        sha,
    })
}

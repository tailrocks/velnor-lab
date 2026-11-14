use crate::Result;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub type Identity = (String, String, String);

#[derive(Debug, PartialEq, Eq)]
pub struct Reach {
    pub literal: BTreeMap<String, BTreeSet<Identity>>,
    pub by_name: BTreeMap<String, BTreeSet<Identity>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Named {
    pub changed: BTreeSet<String>,
    pub version_only: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selected {
    Nothing,
    Some(BTreeSet<String>),
    All(Option<BTreeSet<String>>),
}

pub fn reach(metadata: &Value) -> Result<Reach> {
    let packages: BTreeMap<&str, Identity> = metadata["packages"]
        .as_array()
        .ok_or("metadata has no packages")?
        .iter()
        .filter_map(|package| {
            Some((
                package["id"].as_str()?,
                (
                    package["name"].as_str()?.to_owned(),
                    package["version"].as_str()?.to_owned(),
                    package["source"].as_str().unwrap_or("path").to_owned(),
                ),
            ))
        })
        .collect();
    let members: BTreeSet<&str> = metadata["workspace_members"]
        .as_array()
        .ok_or("metadata has no workspace_members")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let edges: BTreeMap<&str, Vec<&str>> = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("metadata has no resolve; run it without --no-deps")?
        .iter()
        .filter_map(|node| {
            let dependencies = node["dependencies"].as_array()?;
            Some((
                node["id"].as_str()?,
                dependencies.iter().filter_map(Value::as_str).collect(),
            ))
        })
        .collect();
    let mut found = Reach {
        literal: BTreeMap::new(),
        by_name: BTreeMap::new(),
    };
    for member in &members {
        let (name, _, _) = packages.get(member).ok_or("a member is not a package")?;
        let mut seen = BTreeSet::new();
        let mut pending = vec![*member];
        while let Some(id) = pending.pop() {
            if seen.insert(id) {
                pending.extend(edges.get(id).into_iter().flatten());
            }
        }
        let identities = |collapse: bool| -> BTreeSet<Identity> {
            seen.iter()
                .filter_map(|id| {
                    let identity = packages.get(id)?.clone();
                    if collapse && members.contains(id) {
                        Some((identity.0, String::new(), identity.2))
                    } else {
                        Some(identity)
                    }
                })
                .collect()
        };
        found.literal.insert(name.clone(), identities(false));
        found.by_name.insert(name.clone(), identities(true));
    }
    Ok(found)
}

pub fn named(parent: &Reach, head: &Reach) -> Named {
    let differs = |left: &BTreeMap<String, BTreeSet<Identity>>,
                   right: &BTreeMap<String, BTreeSet<Identity>>| {
        right
            .iter()
            .filter(|(member, set)| left.get(*member) != Some(*set))
            .map(|(member, _)| member.clone())
            .collect::<BTreeSet<String>>()
    };
    let changed = differs(&parent.by_name, &head.by_name);
    let version_only = differs(&parent.literal, &head.literal)
        .difference(&changed)
        .cloned()
        .collect();
    Named {
        changed,
        version_only,
    }
}

pub fn members(reach: &Reach) -> BTreeSet<String> {
    reach.literal.keys().cloned().collect()
}

pub fn parse_explain(text: &str) -> BTreeMap<String, Selected> {
    let mut roots = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if let Some(listed) = line.strip_prefix("  ") {
            let package = listed.split(' ').next().unwrap_or_default();
            if let Some(Selected::Some(set) | Selected::All(Some(set))) = current
                .as_ref()
                .and_then(|root| roots.get_mut(root))
                .filter(|_| !listed.starts_with(' '))
            {
                set.insert(package.to_owned());
            }
        } else if let Some((root, header)) = line.rsplit_once(": ")
            && !line.starts_with("unknown path")
        {
            let selected = match header {
                "none" => Selected::Nothing,
                "some" => Selected::Some(BTreeSet::new()),
                header if header.starts_with("all:most-packages") => {
                    Selected::All(Some(BTreeSet::new()))
                }
                _ => Selected::All(None),
            };
            roots.insert(root.to_owned(), selected);
            current = Some(root.to_owned());
        } else {
            current = None;
        }
    }
    roots
}

pub fn missed(
    named: &BTreeSet<String>,
    selected: &Selected,
    members: &BTreeSet<String>,
) -> BTreeSet<String> {
    let chosen = match selected {
        Selected::Nothing => BTreeSet::new(),
        Selected::Some(set) | Selected::All(Some(set)) => set.clone(),
        Selected::All(None) => members.clone(),
    };
    named.difference(&chosen).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn metadata(leaf: &str, fnv: &str, lib_version: &str) -> Value {
        let source = "registry+https://github.com/rust-lang/crates.io-index";
        json!({
            "workspace_members": [format!("path+file:///w/lib#lib@{lib_version}"), "path+file:///w/tool#tool@0.0.0"],
            "packages": [
                {"id": format!("path+file:///w/lib#lib@{lib_version}"), "name": "lib", "version": lib_version, "source": null},
                {"id": "path+file:///w/tool#tool@0.0.0", "name": "tool", "version": "0.0.0", "source": null},
                {"id": format!("{source}#fnv@{fnv}"), "name": "fnv", "version": fnv, "source": source},
                {"id": format!("{source}#{leaf}@1.0.0"), "name": leaf, "version": "1.0.0", "source": source},
            ],
            "resolve": {"nodes": [
                {"id": "path+file:///w/tool#tool@0.0.0", "dependencies": [format!("{source}#fnv@{fnv}"), format!("path+file:///w/lib#lib@{lib_version}")]},
                {"id": format!("path+file:///w/lib#lib@{lib_version}"), "dependencies": [format!("{source}#{leaf}@1.0.0")]},
                {"id": format!("{source}#fnv@{fnv}"), "dependencies": []},
                {"id": format!("{source}#{leaf}@1.0.0"), "dependencies": []},
            ]},
        })
    }

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn a_member_is_named_when_a_package_it_reaches_differs() {
        let parent = reach(&metadata("hex", "1.0.5", "0.0.0")).expect("parent");
        let bumped = reach(&metadata("hex", "1.0.6", "0.0.0")).expect("bumped");
        assert_eq!(members(&parent), set(&["lib", "tool"]));
        assert_eq!(named(&parent, &bumped).changed, set(&["tool"]));
        assert_eq!(named(&parent, &parent).changed, set(&[]));
        let swapped = reach(&metadata("itoa", "1.0.5", "0.0.0")).expect("swapped");
        assert_eq!(named(&parent, &swapped).changed, set(&["lib", "tool"]));
    }

    #[test]
    fn a_members_own_version_is_reported_apart_from_a_changed_dependency() {
        let parent = reach(&metadata("hex", "1.0.5", "0.0.0")).expect("parent");
        let versioned = reach(&metadata("hex", "1.0.5", "0.1.0")).expect("versioned");
        let found = named(&parent, &versioned);
        assert_eq!(found.changed, set(&[]));
        assert_eq!(found.version_only, set(&["lib", "tool"]));
        let both = reach(&metadata("hex", "1.0.6", "0.1.0")).expect("both");
        let found = named(&parent, &both);
        assert_eq!(found.changed, set(&["tool"]));
        assert_eq!(found.version_only, set(&["lib"]));
    }

    #[test]
    fn a_member_that_is_new_at_the_head_is_named() {
        let mut head = reach(&metadata("hex", "1.0.5", "0.0.0")).expect("head");
        let parent = reach(&metadata("hex", "1.0.5", "0.0.0")).expect("parent");
        head.literal.insert("extra".to_owned(), BTreeSet::new());
        head.by_name.insert("extra".to_owned(), BTreeSet::new());
        assert_eq!(named(&parent, &head).changed, set(&["extra"]));
    }

    #[test]
    fn the_explain_output_is_read_per_root_and_a_missed_member_is_one_that_was_not_selected() {
        let text = "Cargo.toml: some\n  tool lock:fnv\nhelper/Cargo.toml: all:most-packages:2\n  a lock:x\n  b lock:x\nother/Cargo.toml: all:workspace-manifest\ntools/Cargo.toml: none\nunknown path: scripts/gen.sh\n  to select nothing, add this line to non_inputs in .velnor/rust-ci.toml:\n    \"scripts/gen.sh\",\n";
        let selections = parse_explain(text);
        assert_eq!(selections["Cargo.toml"], Selected::Some(set(&["tool"])));
        assert_eq!(
            selections["helper/Cargo.toml"],
            Selected::All(Some(set(&["a", "b"])))
        );
        assert_eq!(selections["other/Cargo.toml"], Selected::All(None));
        assert_eq!(selections["tools/Cargo.toml"], Selected::Nothing);
        assert_eq!(selections.len(), 4);
        let all = set(&["lib", "tool"]);
        let named = set(&["lib", "tool"]);
        assert_eq!(
            missed(&named, &selections["Cargo.toml"], &all),
            set(&["lib"])
        );
        assert_eq!(
            missed(&named, &selections["other/Cargo.toml"], &all),
            set(&[])
        );
        assert_eq!(
            missed(&named, &selections["helper/Cargo.toml"], &all),
            named
        );
        assert_eq!(missed(&named, &selections["tools/Cargo.toml"], &all), named);
    }
}

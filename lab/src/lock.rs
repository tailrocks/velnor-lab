use crate::Result;

const CRATE: &str = "fnv";

fn package_range(lock: &str) -> Option<(usize, usize)> {
    let start = lock.find(&format!("name = \"{CRATE}\"\n"))?;
    let end = start + lock[start..].find("\n\n").unwrap_or(lock.len() - start);
    Some((start, end))
}

pub fn current_version(lock: &str) -> Option<String> {
    let (start, end) = package_range(lock)?;
    let line = lock[start..end]
        .lines()
        .find(|l| l.starts_with("version = "))?;
    Some(
        line.trim_start_matches("version = ")
            .trim_matches('"')
            .to_owned(),
    )
}

pub fn next_patch(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    let (major, minor, patch) = (parts.next()?, parts.next()?, parts.next()?);
    Some(format!(
        "{major}.{minor}.{}",
        patch.parse::<u64>().ok()? + 1
    ))
}

pub fn rewrite(lock: &str, version: &str, checksum: &str) -> Option<String> {
    let (start, end) = package_range(lock)?;
    let block: Vec<String> = lock[start..end]
        .lines()
        .map(|line| match line.split_once(" = ") {
            Some(("version", _)) => format!("version = \"{version}\""),
            Some(("checksum", _)) => format!("checksum = \"{checksum}\""),
            _ => line.to_owned(),
        })
        .collect();
    Some(format!(
        "{}{}{}",
        &lock[..start],
        block.join("\n"),
        &lock[end..]
    ))
}

fn checksum(version: &str) -> Result<String> {
    let url = format!("https://index.crates.io/3/f/{CRATE}");
    let body = ureq::get(&url).call()?.body_mut().read_to_string()?;
    let needle = format!("\"vers\":\"{version}\"");
    let line = body
        .lines()
        .find(|l| l.contains(&needle) && l.contains("\"yanked\":false"));
    let line = line.ok_or_else(|| format!("{CRATE} {version} is not on crates.io"))?;
    let value: serde_json::Value = serde_json::from_str(line)?;
    Ok(value["cksum"]
        .as_str()
        .ok_or("index line has no cksum")?
        .to_owned())
}

pub fn bump(lock: &str) -> Result<String> {
    let current = current_version(lock).ok_or("Cargo.lock has no fnv entry")?;
    let next = next_patch(&current).ok_or("fnv version is not x.y.z")?;
    let sum = checksum(&next)?;
    rewrite(lock, &next, &sum).ok_or_else(|| "fnv entry not found".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = "[[package]]\nname = \"fnv\"\nversion = \"1.0.5\"\nsource = \"registry+x\"\nchecksum = \"aa\"\n\n[[package]]\nname = \"hex\"\nversion = \"0.4.3\"\n";

    #[test]
    fn rewrites_only_the_fnv_entry() {
        let out = rewrite(LOCK, "1.0.6", "bb").expect("entry");
        assert!(
            out.contains("version = \"1.0.6\"\nsource = \"registry+x\"\nchecksum = \"bb\"\n\n")
        );
        assert!(out.contains("name = \"hex\"\nversion = \"0.4.3\""));
    }

    #[test]
    fn reads_and_bumps_the_patch() {
        assert_eq!(current_version(LOCK).as_deref(), Some("1.0.5"));
        assert_eq!(next_patch("1.0.5").as_deref(), Some("1.0.6"));
    }
}

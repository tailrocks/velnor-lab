use crate::Result;
use crate::api::Api;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Opened {
    pub number: u64,
    pub url: String,
    pub branch: String,
    pub sha: String,
}

pub fn tag() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!("{secs}-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

pub fn retire(api: &Api, opened: &Opened) -> Result<()> {
    let state = json!({"state": "closed"});
    let _ = api.send("PATCH", &format!("pulls/{}", opened.number), &state);
    retire_branch(api, opened)
}

pub fn retire_branch(api: &Api, opened: &Opened) -> Result<()> {
    keep(api, opened)?;
    match api.delete(&format!("git/refs/heads/{}", opened.branch)) {
        Ok(_) => Ok(()),
        Err(error) if is_missing_ref(&error.to_string()) => Ok(()),
        Err(error) => Err(error),
    }
}

fn keep(api: &Api, opened: &Opened) -> Result<()> {
    let body = json!({"ref": format!("refs/keep/{}", opened.branch), "sha": opened.sha});
    let Err(error) = api.send("POST", "git/refs", &body) else {
        return Ok(());
    };
    let existing: Option<Value> = api.get(&format!("git/ref/keep/{}", opened.branch))?;
    match existing {
        Some(found) if found["object"]["sha"] == opened.sha.as_str() => Ok(()),
        _ => Err(error),
    }
}

fn is_missing_ref(message: &str) -> bool {
    message.contains("HTTP 422") && message.contains("Reference does not exist")
}

#[cfg(test)]
mod tests {
    use super::is_missing_ref;

    #[test]
    fn only_a_422_for_a_missing_reference_counts_as_already_deleted() {
        let gone = r#"git/refs/heads/lab/warm-1: HTTP 422: {"message":"Reference does not exist","status":"422"}"#;
        let other = r#"git/refs/heads/lab/warm-1: HTTP 422: {"message":"Reference update failed","status":"422"}"#;
        let denied = "git/refs/heads/lab/warm-1: HTTP 403: Reference does not exist";
        assert!(is_missing_ref(gone));
        assert!(!is_missing_ref(other));
        assert!(!is_missing_ref(denied));
    }
}

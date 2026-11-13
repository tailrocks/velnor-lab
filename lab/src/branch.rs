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
    let keep = json!({"ref": format!("refs/keep/{}", opened.branch), "sha": opened.sha});
    let _ = api.send("POST", "git/refs", &keep);
    if api
        .get(&format!("git/ref/heads/{}", opened.branch))?
        .is_some()
    {
        let _: Option<Value> = api.delete(&format!("git/refs/heads/{}", opened.branch))?;
    }
    Ok(())
}

use crate::Result;
use serde_json::Value;
use ureq::Agent;

pub struct Api {
    agent: Agent,
    token: String,
    pub repo: String,
}

impl Api {
    pub fn from_env() -> Result<Self> {
        let token = std::env::var("GH_TOKEN").map_err(|_| "GH_TOKEN is not set")?;
        let repo = std::env::var("LAB_REPO").unwrap_or_else(|_| "tailrocks/velnor-lab".into());
        let agent = Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .into();
        Ok(Self { agent, token, repo })
    }

    pub fn base(&self) -> String {
        std::env::var("LAB_BASE").unwrap_or_else(|_| "main".into())
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    fn url(&self, path: &str) -> String {
        if path.starts_with("https://api.github.com/") {
            return path.to_owned();
        }
        match path.strip_prefix('/') {
            Some(rest) => format!("https://api.github.com/{rest}"),
            None => format!("https://api.github.com/repos/{}/{path}", self.repo),
        }
    }

    fn auth(&self) -> String {
        format!("Bearer {}", self.token)
    }

    pub fn get(&self, path: &str) -> Result<Option<Value>> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            let sent = self
                .agent
                .get(self.url(path))
                .header("Authorization", self.auth())
                .header("User-Agent", "velnor-lab")
                .call();
            match sent {
                Ok(response) => return finish(response, path),
                Err(_) if attempt < 3 => std::thread::sleep(std::time::Duration::from_secs(2)),
                Err(error) => return Err(error.into()),
            }
        }
    }

    pub fn download(&self, path: &str) -> Result<Vec<u8>> {
        let mut response = self
            .agent
            .get(self.url(path))
            .header("Authorization", self.auth())
            .header("User-Agent", "velnor-lab")
            .call()?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(format!("{path}: HTTP {status}").into());
        }
        Ok(response.body_mut().read_to_vec()?)
    }

    pub fn delete(&self, path: &str) -> Result<Option<Value>> {
        let request = self.agent.delete(self.url(path));
        finish(
            request
                .header("Authorization", self.auth())
                .header("User-Agent", "velnor-lab")
                .call()?,
            path,
        )
    }

    pub fn send(&self, method: &str, path: &str, body: &Value) -> Result<Value> {
        let builder = match method {
            "POST" => self.agent.post(self.url(path)),
            "PUT" => self.agent.put(self.url(path)),
            _ => self.agent.patch(self.url(path)),
        };
        let response = builder
            .header("Authorization", self.auth())
            .header("User-Agent", "velnor-lab")
            .send_json(body)?;
        finish(response, path)?.ok_or_else(|| format!("{path}: not found").into())
    }
}

fn finish(mut response: ureq::http::Response<ureq::Body>, path: &str) -> Result<Option<Value>> {
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string()?;
    match status {
        404 => Ok(None),
        200..=299 if text.is_empty() => Ok(Some(Value::Null)),
        200..=299 => Ok(Some(serde_json::from_str(&text)?)),
        _ => Err(format!("{path}: HTTP {status}: {text}").into()),
    }
}

use crate::Result;
use crate::api::Api;

pub fn get(api: &Api) -> Result<(String, String)> {
    let text = match std::env::var("LAB_SIGNER") {
        Ok(text) => text,
        Err(_) => {
            let user = api
                .get("/user")?
                .ok_or("cannot read /user; set LAB_SIGNER")?;
            let login = user["login"].as_str().unwrap_or("lab");
            let name = user["name"].as_str().unwrap_or(login);
            let email = user["email"].as_str().map_or_else(
                || format!("{}+{login}@users.noreply.github.com", user["id"]),
                str::to_owned,
            );
            format!("{name} <{email}>")
        }
    };
    let (name, email) = text
        .split_once(" <")
        .ok_or("LAB_SIGNER must be `Name <email>`")?;
    Ok((name.to_owned(), email.trim_end_matches('>').to_owned()))
}

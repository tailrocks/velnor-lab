mod api;
mod branch;
mod grow;
mod lock;
mod matrix;
mod scenario;
mod signer;
mod text;
mod wait;

use api::Api;
use scenario::Scenario;
use std::process::ExitCode;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn run(args: &[String]) -> Result<()> {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["grow", count] => grow::run(count.parse()?),
        ["matrix", workflow] => matrix::run(&Api::from_env()?, workflow),
        ["merge", number] => {
            let api = Api::from_env()?;
            wait::merge(&api, number.parse()?).map(|sha| println!("merged {sha}"))
        }
        ["cold"] => matrix::cold(&Api::from_env()?).map(|_| ()),
        [name] if Scenario::parse(name).is_some() => {
            let scenario = Scenario::parse(name).ok_or("unknown scenario")?;
            let opened = scenario::open(&Api::from_env()?, scenario)?;
            println!("{} {}", opened.number, opened.url);
            Ok(())
        }
        _ => Err(
            "usage: lab <warm|cold|docs|leaf|hub|lock> | merge <pr> | matrix <workflow> | grow <n>"
                .into(),
        ),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("lab: {error}");
            ExitCode::FAILURE
        }
    }
}

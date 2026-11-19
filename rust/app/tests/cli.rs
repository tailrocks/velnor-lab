use std::process::Command;

fn run(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_app"))
        .args(args)
        .output()
        .expect("app runs");
    assert!(output.status.success());
    String::from_utf8(output.stdout).expect("utf8")
}

#[test]
fn prints_the_greeting_for_a_name() {
    let out = run(&["--name", "lab", "--id", "5"]);
    assert_eq!(out.trim(), "hello from shared data | lib:5:lab");
}

#[test]
fn defaults_to_world() {
    assert!(run(&[]).trim().ends_with("lib:1:world"));
}

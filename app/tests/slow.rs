use std::thread::sleep;
use std::time::{Duration, Instant};

fn wait_twenty_seconds() {
    let start = Instant::now();
    sleep(Duration::from_secs(20));
    assert!(start.elapsed() >= Duration::from_secs(20));
}

#[test]
fn slow_one() {
    wait_twenty_seconds();
}

#[test]
fn slow_two() {
    wait_twenty_seconds();
}

#[test]
fn slow_three() {
    wait_twenty_seconds();
}

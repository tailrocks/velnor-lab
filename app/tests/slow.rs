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

#[test]
fn slow_four() {
    wait_twenty_seconds();
}

#[test]
fn slow_five() {
    wait_twenty_seconds();
}

#[test]
fn slow_six() {
    wait_twenty_seconds();
}

#[test]
fn slow_seven() {
    wait_twenty_seconds();
}

#[test]
fn slow_eight() {
    wait_twenty_seconds();
}

#[test]
fn slow_nine() {
    wait_twenty_seconds();
}

#[test]
fn slow_ten() {
    wait_twenty_seconds();
}

#[test]
fn slow_eleven() {
    wait_twenty_seconds();
}

#[test]
fn slow_twelve() {
    wait_twenty_seconds();
}
// lab/slow12-1794557204

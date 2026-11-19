use std::thread::sleep;
use std::time::{Duration, Instant};

// proof 622: a deliberately slow test, reverted right after the budget check names it.
#[test]
fn budget_breaching_slow_test() {
    let start = Instant::now();
    sleep(Duration::from_secs(150));
    assert!(start.elapsed() >= Duration::from_secs(150));
}

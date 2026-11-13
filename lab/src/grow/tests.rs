use super::*;

#[test]
fn chain_and_star_dependencies() {
    assert!(manifest(0).ends_with("[dependencies]\n"));
    assert!(manifest(1).contains("grow-000 = { path = \"../g000\" }"));
    let third = manifest(5);
    assert!(third.contains("grow-004") && third.contains("grow-000"));
}

#[test]
fn source_calls_its_dependencies_and_fits_the_size() {
    let text = source(7);
    assert!(text.contains("grow_006::total(value)") && text.contains("grow_000::total(value)"));
    assert!(text.lines().count() < 60 && !text.contains("{ID}") && !text.contains("{CALLS}"));
}

#[test]
fn total_is_written_the_way_rustfmt_writes_it() {
    assert!(source(0).contains("pub fn total(value: u64) -> u64 {\n    mix(value)\n}\n"));
    assert!(source(1).contains(
        "pub fn total(value: u64) -> u64 {\n    mix(value).wrapping_add(grow_000::total(value))\n}\n"
    ));
    assert!(source(7).contains(
        "pub fn total(value: u64) -> u64 {\n    mix(value)\n        .wrapping_add(grow_006::total(value))\n        .wrapping_add(grow_000::total(value))\n}\n"
    ));
}

#[test]
fn member_glob_lands_inside_the_list() {
    let out = add_member("[workspace]\nmembers = [\n  \"a\",\n]\nx = 1\n").expect("list");
    assert_eq!(
        out,
        "[workspace]\nmembers = [\n  \"a\",\n  \"grow/*\",\n]\nx = 1\n"
    );
}

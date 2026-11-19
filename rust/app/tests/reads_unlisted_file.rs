// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

#[test]
fn reads_a_tracked_file_outside_every_rust_path() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/notes.md");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("open {}: {error}", path.display()));
    assert!(text.contains("Lab notes"));
}

# velnor-lab

A small Rust project used to try CI ideas quickly and to run the Velnor Rust CI generator for real.

It has two jobs:

- **Lab.** A workflow or cache idea is tried here first, because a run takes a minute or two.
- **First user.** The workflow that `velnor-gen` writes for this project runs here every night on real GitHub.

The project is kept small on purpose. Each crate exists to exercise one thing a CI generator must handle. It holds no product code.

The plan this repository serves is the issue tree under [tailrocks/velnor-new#574](https://github.com/tailrocks/velnor-new/issues/574). The task that builds this project is [tailrocks/velnor-new#584](https://github.com/tailrocks/velnor-new/issues/584).

## Crates

| Path | Purpose |
|---|---|
| `core` | The hub. Everything depends on it. Uses `serde` with `derive` |
| `lib` | Depends on `core` and `macros`. Has a `build.rs` with `cargo:rerun-if-changed` |
| `app` | The leaf binary. Nothing depends on it. Reads `shared-data/` with `include_str!`, runs through `CARGO_BIN_EXE_app` in a test, and has three 20 s tests in `tests/slow.rs` |
| `tool` | Standalone binary. Shares `hex` with `lib` under another feature set, and is the only user of `fnv` |
| `macros` | A proc-macro crate used by `lib` |
| `shared-data/` | Not a crate. One text file that `app` includes |

The branch `specimen/fails-alone` is never merged. On it `cargo check -p lib` fails and `cargo check --workspace` passes.

## Rules

- Changes reach `main` through pull requests, squash-merged, with every review conversation resolved.
- Commits are signed off (`git commit -s`).
- Licence: Apache-2.0. See `LICENSE`, `NOTICE` and `REUSE.toml`.

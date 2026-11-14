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
| `lib` | Depends on `core` and `macros`. Has a `build.rs` that reads `shared-data/build-name.txt` and names it with `cargo:rerun-if-changed` |
| `app` | The leaf binary. Nothing depends on it. Reads `shared-data/` with `include_str!`, runs through `CARGO_BIN_EXE_app` in a test, and has three 20 s tests in `tests/slow.rs` |
| `tool` | Standalone binary. Shares `hex` with `lib` under another feature set, and is the only user of `fnv` |
| `macros` | A proc-macro crate used by `lib` |
| `shared-data/` | Not a crate. `greeting.txt`, which `app` includes, and `build-name.txt`, which the build script of `lib` reads |
| `lab/` | The scenario driver. Its own workspace, not part of the one under test |

The branch `specimen/fails-alone` is never merged. On it `cargo check -p lib` fails and `cargo check --workspace` passes.

## Run a scenario

Tools come from `mise install`. The driver needs `GH_TOKEN` in the environment and `LAB_SIGNER="Name <email>"` when the token cannot read `/user`. Every scenario is a pull request with one signed-off commit.

```sh
cargo run --manifest-path lab/Cargo.toml -- <scenario>
```

| Scenario | What it does |
|---|---|
| `warm` | Appends a comment to `rust-toolchain.toml`. Every lane runs, nothing should compile |
| `cold` | Deletes every cache, then merges a `warm` pull request once its checks are green |
| `merge <pr>` | Squash-merges a pull request once its checks are green |
| `docs` | Appends a line to `docs/notes.md` |
| `leaf` | Appends a comment to `app/src/main.rs` |
| `hub` | Appends a comment to `core/src/lib.rs` |
| `lock` | Bumps `fnv` in `Cargo.lock` to its next patch version |
| `matrix <workflow>` | Runs `cold`, 3 `warm`, 3 `leaf`, `hub`, `lock`, `docs`, and writes `velnor-ci bench collect` output to `results/<workflow>/` |
| `fault-test` | Adds to `core/src/lib.rs` a test whose assertion fails |
| `fault-type` | Adds to `core/src/lib.rs` a function with a type error |
| `fault-borrow` | Adds to `core/src/lib.rs` a function that uses a value after moving it |
| `fault-lint` | Adds to `core/src/lib.rs` a function that takes `&Vec<u32>`, which clippy's `ptr_arg` rejects |
| `runs <dir> <workflow> <scenario> <n>` | Runs one scenario `n` times against one workflow and writes, per run, the `velnor-ci bench collect` output (successful runs only), the API's run and jobs JSON, and the job logs to `<dir>/<workflow>/`. The scenario is any of the above, `cold`, or `leaf2`: a `leaf` pull request followed by a second `leaf` push to the same branch |
| `grow <n>` | On a local branch, adds `n` crates under `grow/` in a chain and star pattern |
| `replay-lock <checkout> [count] [--all]` | For each of the newest `count` (default 50) commits of `<checkout>` that change a `Cargo.lock`, `--all` counting commits of every ref, compares the members that `velnor-ci plan` selects for the lock change (`--changed` and `--base-file` only) with the members whose reachable packages (name, version, source) differ between `cargo metadata --locked` at the parent and at the commit. A member's own version is compared by name only and reported apart. Prints one line per lock file, the commits it could not resolve with the reason, and a summary; exits 1 when a member is missed. `VELNOR_CI` names the helper binary and `CARGO` the cargo binary. A commit without a tracked `.velnor/rust-ci.toml` gets one that excludes the manifests cargo cannot load |

| `probes [--check] [probe...]` | Runs the conformance probes below, downloads their artifacts and writes `conformance/<probe>.json`. With `--check` it writes nothing and fails naming each probe whose observation differs from the stored file |

`LAB_REPO` (default `tailrocks/velnor-lab`) and `LAB_BASE` (default `main`) select the repository and base branch. `VELNOR_CI` names the helper binary for `matrix`.

`.github/workflows/` holds the hand-written workflows of the experiment [tailrocks/velnor-new#585](https://github.com/tailrocks/velnor-new/issues/585), one per question. `blaze-signal.yml`, `blaze-signal-state.yml` and `blaze-routing.yml` belong to the experiment [tailrocks/velnor-new#652](https://github.com/tailrocks/velnor-new/issues/652); `.github/blaze.sha256` pins the blaze archives they download. Only the workflow under test is enabled while its `matrix` runs; the branch `control` holds the engine's generated workflow.

## Conformance probes

`.github/workflows/probe-*.yml` record how GitHub behaves where [tailrocks/velnor-new#633](https://github.com/tailrocks/velnor-new/issues/633) relies on it. Each run writes one observation as JSON and uploads it as the artifact `observation`. `conformance/<probe>.json` holds the stored observations, one entry per leg. Values that change between runs (tags, run ids, pull request numbers) are replaced by placeholders.

| Probe | Observes | Legs |
|---|---|---|
| `token` | scope names and permissions (`ac` claim), `scp` names and the other claim names of the runtime token. Never the token or its signature | one per event |
| `cache-save-existing` | what the save step reports for a key that exists in the same scope | `default-branch` |
| `cache-restore-keys` | which entry a key, a key prefix and restore keys restore when entries share a prefix | `default-branch` |
| `cache-pr-reads` | whether a pull request restores entries saved on the default branch, its base branch and its own head branch, and which of two entries with the same key (one on the default branch, one on the base branch, different content) it restores | `pull_request` |
| `cache-other-pr` | whether a pull request restores an entry saved by another pull request, after a control in which the first pull request restores its own save | `first_pull_request`, `second_pull_request` |
| `cache-own-pr` | whether a second run of a pull request restores what its first run saved | `run-1`, `run-2` |
| `needs-result` | `needs.<job>.result` and `toJSON(needs)` after a skipped job and a cancelled job | `default-branch` |
| `event-payload` | the payload fields and environment variables a helper may read, per event | one per event |

The workflows with a `schedule` trigger (`token`, `event-payload`, `cache-save-existing`, `cache-restore-keys`) compare their observation with the stored file every week and fail naming the probe. A leg that is not stored yet gives a warning and does not fail. `needs-result` has no schedule: it cancels its own run, so a failed comparison could not turn the run red; `lab probes --check needs-result` covers it. The probes that need a pull request (`cache-pr-reads`, `cache-other-pr`, `cache-own-pr`) and the `push` and `pull_request` legs of `token` and `event-payload` need runs that `GITHUB_TOKEN` cannot start, so only `lab probes --check` with a personal token checks them. `schedule` is the only event a person cannot start, so the `schedule` legs come from the latest scheduled run, which `lab probes` downloads.

Probe pull requests use branches `probe-<tag>-<role>`. `lab probes` closes them, saves each branch as `refs/keep/<branch>` and deletes it.

## Rules

- Changes reach `main` through pull requests, squash-merged, with every review conversation resolved.
- Commits are signed off (`git commit -s`).
- Licence: Apache-2.0. See `LICENSE`, `NOTICE` and `REUSE.toml`.

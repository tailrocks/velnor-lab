// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

"use strict";

import { chmod, lstat, mkdir, mkdtemp, readdir, realpath, rm, stat } from "node:fs/promises";
import { join, resolve } from "node:path";
import { cpus, loadavg, platform, release, tmpdir } from "node:os";
import { createHash } from "node:crypto";

interface Fixture {
  stdout: string;
  stderr: string;
  artifacts: number;
  tests: number;
}
interface Inventory {
  archiveCount: number;
  registryDirectories: string[];
  bytes: number;
  fingerprint: string;
  gitDb: { state: "absent"; path: string; entryCount: 0; bytes: 0; directories: []; fingerprint: string };
}
type Variant = "baseline" | "candidate";
interface Trial {
  pair: number;
  order: Variant[];
  baselineMs: number;
  candidateMs: number;
  differenceMs: number;
}
interface Context {
  root: string;
  cargo: string;
  cargoHome: string;
  fixture: Fixture;
  binaries: Record<Variant, string>;
}

function requireCondition(value: boolean, message: string): asserts value {
  if (!value) throw new Error(message);
}

function object(value: unknown): Record<string, unknown> {
  requireCondition(typeof value === "object" && value !== null && !Array.isArray(value), "expected JSON object");
  return Object.fromEntries(Object.entries(value));
}

function fixture(): Fixture {
  const artifacts = 200;
  const tests = 17;
  const stdout = Array.from({ length: artifacts }, (_, index) => JSON.stringify({
    reason: "compiler-artifact",
    package_id: `registry+https://github.com/rust-lang/crates.io-index#replay_${index}@0.1.0`,
    manifest_path: `/fixture/registry/replay_${index}/Cargo.toml`,
    target: {
      name: `replay_${index}`, kind: ["lib"], crate_types: ["rlib"],
      src_path: `/fixture/registry/replay_${index}/src/lib.rs`,
      edition: "2024", doc: true, doctest: true, test: true,
    },
    profile: { opt_level: "0", debuginfo: 2, debug_assertions: true, overflow_checks: true, test: false },
    features: ["default"], filenames: [`target/debug/deps/libreplay_${index}-0123456789abcdef.rlib`],
    executable: null, fresh: true,
  })).join("\n") + "\n";
  const stderr = Array.from({ length: tests }, (_, index) => `        PASS [0.001s] replay test_${index}`).join("\n")
    + "\n     Summary [0.017s] 17 tests run: 17 passed, 0 skipped\n";
  return { stdout, stderr, artifacts, tests };
}

async function inventory(cargoHome: string): Promise<Inventory> {
  const gitDbPath = join(cargoHome, "git/db");
  let gitDbAbsent = false;
  try {
    await lstat(gitDbPath);
  } catch (error: unknown) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") gitDbAbsent = true;
    else throw error;
  }
  requireCondition(gitDbAbsent, "replay requires actual built-job git/db absence; populated or empty Git databases are outside this bounded experiment");
  const cache = join(cargoHome, "registry/cache");
  const registryDirectories: string[] = [];
  const stamps: string[] = [];
  let bytes = 0;
  for (const registry of (await readdir(cache)).sort()) {
    const directory = join(cache, registry);
    if (!(await stat(directory)).isDirectory()) continue;
    registryDirectories.push(registry);
    for (const name of (await readdir(directory)).sort()) {
      if (!name.endsWith(".crate")) continue;
      const metadata = await stat(join(directory, name));
      if (!metadata.isFile()) continue;
      bytes += metadata.size;
      stamps.push(`${registry}/${name}\t${metadata.size}\t${metadata.mtimeMs}`);
    }
  }
  requireCondition(stamps.length > 0, "CARGO_HOME must contain actual built-job registry archives");
  return {
    archiveCount: stamps.length, registryDirectories, bytes,
    fingerprint: createHash("sha256").update(stamps.join("\n")).digest("hex"),
    gitDb: { state: "absent", path: gitDbPath, entryCount: 0, bytes: 0, directories: [],
      fingerprint: createHash("sha256").update("git/db absent").digest("hex") },
  };
}

function childEnvironment(context: Context, directory: string): Record<string, string> {
  const environment: Record<string, string> = {};
  for (const [key, value] of Object.entries(process.env)) {
    if (value === undefined) continue;
    if (/^(GITHUB_|ACTIONS_|RUNNER_|VELNOR_)/.test(key)) continue;
    if (/(TOKEN|SECRET|PASSWORD|CREDENTIAL|PRIVATE_KEY|ACCESS_KEY|AUTHORIZATION)/i.test(key)) continue;
    environment[key] = value;
  }
  return {
    ...environment, CARGO_HOME: context.cargoHome, RUNNER_TEMP: directory,
    CARGO_TARGET_DIR: join(directory, "target"), VELNOR_CI_RUN_TERMINAL: "0",
    REPLAY_STDOUT: join(context.root, "stdout"), REPLAY_STDERR: join(context.root, "stderr"),
  };
}

async function validate(context: Context, directory: string, variant: Variant, exit: number): Promise<void> {
  requireCondition(exit === 0, `${variant}: helper process failed with exit ${exit}`);
  const log = await Bun.file(join(directory, "velnor-run.log")).text();
  requireCondition(log.includes(context.fixture.stderr), `${variant}: log missing PASS timings or nextest summary`);
  requireCondition(log.replace(context.fixture.stderr, "").includes(context.fixture.stdout), `${variant}: log missing artifact payload`);
  requireCondition(log.endsWith("velnor-ci run: exit 0\n"), `${variant}: missing successful log terminator`);
  const sidecar = Bun.file(join(directory, "velnor-invocations.jsonl"));
  if (variant === "baseline") {
    requireCondition(!(await sidecar.exists()), "baseline unexpectedly produced capture sidecar");
    return;
  }
  requireCondition(await sidecar.exists(), "candidate missing capture sidecar (feature absent)");
  const lines = (await sidecar.text()).trim().split("\n");
  requireCondition(lines.length === 1, "candidate expected exactly one invocation");
  const parsed: unknown = JSON.parse(lines[0] ?? "");
  const record = object(parsed);
  requireCondition(record.capture_complete === true && record.rejected === 0, "candidate capture incomplete or rejected artifacts");
  requireCondition(Array.isArray(record.artifacts) && record.artifacts.length === context.fixture.artifacts, "candidate artifact count mismatch");
  requireCondition(Array.isArray(record.tests) && record.tests.length === context.fixture.tests, "candidate test count mismatch");
  const expectedArtifacts = context.fixture.stdout.trim().split("\n").map((line) => object(JSON.parse(line)));
  for (const [index, artifact] of record.artifacts.entries()) {
    const actual = object(artifact);
    const expected = expectedArtifacts[index];
    requireCondition(expected !== undefined && actual.package_id === expected.package_id && actual.fresh === true,
      "candidate artifact identity or fresh flag mismatch");
  }
  for (const [index, test] of record.tests.entries()) {
    requireCondition(Array.isArray(test) && test[0] === `replay test_${index}` && test[1] === 0.001,
      "candidate test identity or duration mismatch");
  }
  requireCondition(Array.isArray(record.verified_download_archives) && record.verified_download_archives.length === 0,
    "candidate archive scan failed or observed unexpected changes");
  requireCondition(Array.isArray(record.verified_git_downloads) && record.verified_git_downloads.length === 0,
    "candidate Git database observation failed or observed unexpected changes");
}

async function run(context: Context, variant: Variant, prefix: string[] = []): Promise<number> {
  const directory = join(context.root, variant);
  await rm(directory, { recursive: true, force: true });
  await mkdir(directory, { mode: 0o700 });
  const started = performance.now();
  const child = Bun.spawn([...prefix, context.binaries[variant], "run", "--", context.cargo, "nextest", "run"], {
    cwd: directory, env: childEnvironment(context, directory), stdout: "ignore", stderr: "ignore",
  });
  const exit = await child.exited;
  const elapsed = performance.now() - started;
  await validate(context, directory, variant, exit);
  return elapsed;
}

function summary(values: number[]): Record<string, number> {
  requireCondition(values.length > 0, "empty measurements");
  const sorted = [...values].sort((a, b) => a - b);
  const at = (index: number): number => {
    const value = sorted[index];
    requireCondition(value !== undefined, "invalid quantile index");
    return value;
  };
  const median = (at(Math.floor((sorted.length - 1) / 2)) + at(Math.floor(sorted.length / 2))) / 2;
  const deviations = values.map((value) => Math.abs(value - median)).sort((a, b) => a - b);
  const mad = ((deviations[Math.floor((values.length - 1) / 2)] ?? 0) + (deviations[Math.floor(values.length / 2)] ?? 0)) / 2;
  return { median, min: at(0), max: at(sorted.length - 1), p90: at(Math.ceil(sorted.length * 0.9) - 1), mad };
}

async function commandText(command: string[]): Promise<string> {
  const child = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exit] = await Promise.all([
    new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited,
  ]);
  requireCondition(exit === 0, `${command[0]} metadata command failed (${exit})`);
  return (stdout + stderr).trim();
}

async function profile(context: Context): Promise<Record<string, unknown>> {
  const strace = Bun.which("strace");
  if (platform() !== "linux" || strace === null) {
    return { status: "unavailable", consequence: "inconclusive until an unreported profile identifies the limiter" };
  }
  const reports: Record<string, string> = {};
  for (const variant of ["baseline", "candidate"] satisfies Variant[]) {
    const path = join(context.root, `${variant}-strace.txt`);
    await run(context, variant, [strace, "-f", "-c", "-o", path]);
    reports[variant] = await Bun.file(path).text();
  }
  return { status: "collected_outside_trials", syscallSummaries: reports,
    interpretation: "Review file metadata, read/write, process/thread startup and wait syscalls against archive_witness.rs, pump.rs and recording.rs; syscall totals alone do not establish a CPU parsing limiter." };
}

async function main(): Promise<void> {
  const [baseline, candidate, output, ...extra] = Bun.argv.slice(2);
  requireCondition(baseline !== undefined && candidate !== undefined && output !== undefined && extra.length === 0,
    "usage: bun report-overhead.ts BASELINE_BIN CANDIDATE_BIN OUTPUT_JSON");
  const cargoHome = await realpath(process.env.CARGO_HOME ?? join(process.env.HOME ?? "", ".cargo"));
  const before = await inventory(cargoHome);
  const root = await mkdtemp(join(process.env.TMPDIR ?? tmpdir(), "velnor-report-overhead-"));
  await chmod(root, 0o700);
  try {
    const payload = fixture();
    await Bun.write(join(root, "stdout"), payload.stdout);
    await Bun.write(join(root, "stderr"), payload.stderr);
    const cargo = join(root, "cargo");
    await Bun.write(cargo, '#!/bin/sh\nset -eu\n[ "$#" -eq 2 ] && [ "$1" = nextest ] && [ "$2" = run ] || exit 93\n/bin/cat "$REPLAY_STDOUT"\n/bin/cat "$REPLAY_STDERR" >&2\n');
    await chmod(cargo, 0o700);
    const context: Context = { root, cargo, cargoHome, fixture: payload,
      binaries: { baseline: await realpath(baseline), candidate: await realpath(candidate) } };
    const metadata = {
      platform: platform(), release: release(), cpuCount: cpus().length, cpuModel: cpus()[0]?.model,
      loadAverageBefore: loadavg(), uptime: await commandText(["uptime"]),
      cpuInfo: platform() === "linux" ? await Bun.file("/proc/cpuinfo").text() : "unavailable",
      bun: Bun.version, rust: await commandText(["rustc", "--version"]),
      nextest: await commandText(["cargo-nextest", "--version"]),
      binaryVersions: { baseline: await commandText([context.binaries.baseline, "--version"]),
        candidate: await commandText([context.binaries.candidate, "--version"]) },
      sourceBaselineSha: process.env.SOURCE_BASELINE_SHA ?? null,
      sourceCandidateSha: process.env.SOURCE_CANDIDATE_SHA ?? null,
      sourceIdentityNote: "SHAs supplied by caller; --version does not prove source identity",
    };
    console.log(`Replay workload: ${payload.artifacts} fresh artifacts, ${payload.tests} tests, ${before.archiveCount} existing archives`);
    const profiling = await profile(context);
    console.log("Validating three warmup pairs");
    for (let index = 0; index < 3; index += 1) {
      await run(context, "baseline");
      await run(context, "candidate");
    }
    const trials: Trial[] = [];
    console.log("Measuring twenty alternating pairs");
    for (let pair = 0; pair < 20; pair += 1) {
      const order: Variant[] = pair % 2 === 0 ? ["baseline", "candidate"] : ["candidate", "baseline"];
      const times: Record<Variant, number> = { baseline: 0, candidate: 0 };
      for (const variant of order) times[variant] = await run(context, variant);
      trials.push({ pair, order, baselineMs: times.baseline, candidateMs: times.candidate,
        differenceMs: times.candidate - times.baseline });
    }
    const after = await inventory(cargoHome);
    requireCondition(before.fingerprint === after.fingerprint, "registry archive inventory changed during replay");
    requireCondition(before.gitDb.fingerprint === after.gitDb.fingerprint, "Git database inventory changed during replay");
    const baselineStats = summary(trials.map((trial) => trial.baselineMs));
    const candidateStats = summary(trials.map((trial) => trial.candidateMs));
    const difference = summary(trials.map((trial) => trial.differenceMs));
    await Bun.write(resolve(output), JSON.stringify({
      schema: 1, workload: "synthetic warm-output replay, no Rust compilation or downloads",
      expectedClaim: "candidate minus baseline elapsed milliseconds per replay invocation on this archive inventory",
      decision: "Owner evaluates measured capture cost separately from report/upload cost; this script does not accept a regression",
      scope: "JavaScript child-environment copy, helper output processing, before/after registry archive stat scans and absent-Git-database observations, serialization, and OS process/thread startup",
      exclusions: "compiler, cache restoration, mise, network, queue time, report generation and upload; no full-CI extrapolation or extrapolation to populated Git databases",
      metadata, inventory: before, fixture: { artifacts: payload.artifacts, tests: payload.tests,
        stdoutBytes: Buffer.byteLength(payload.stdout), stderrBytes: Buffer.byteLength(payload.stderr),
        rationale: "200 artifacts is within observed 100-240 warm command outputs; fixed 17-test summary exercises existing SummaryTap" },
      warmupPairs: 3, measuredPairs: trials.length, order: "deterministic AB/BA alternating, no random seed",
      successfulMeasuredInvocations: trials.length * 2, failedMeasuredInvocations: 0,
      statisticsMs: { baseline: baselineStats, candidate: candidateStats, pairedCandidateMinusBaseline: difference },
      noiseRule: "Do not claim a difference when paired variation exceeds the effect; no further samples without a new hypothesis",
      verdict: "requires profile and spread review; synthetic capture cost only", profiling,
      loadAverageAfter: loadavg(), trials,
    }, null, 2) + "\n");
    console.log(`Validated all invocations; wrote ${resolve(output)}`);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

await main();

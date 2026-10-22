# Test and Verification Reference

## Summary

This reference defines the four commands that verify the `excalidraw-spatial` crate from the repository root, the output and exit status each command produces on success and failure, and the timestamp override that keeps integration-test JSON deterministic. Use it when running the local check suite or diagnosing a failing check. For CLI exit codes and the `EXCAL_FIXED_NOW_MS` contract, see [`docs/cli-reference.md`](cli-reference.md); for the crate invariants under test, see [`docs/architecture.md`](architecture.md).

## Table of Contents

- [Overview](#overview)
- [Contract and Behavior](#contract-and-behavior)
- [Failures and Limits](#failures-and-limits)
- [Further Exploration](#further-exploration)
- [Dev Note](#dev-note)

---

## Overview

Run every command from the repository root. The suite needs a Rust toolchain with the `rustfmt` and `clippy` components; `cargo test` uses the standard test harness. The commands are independent; none rewrites source files, and a clean verification run requires each command to exit `0`.

| Command                                        | Checks                                                                          |
| ---------------------------------------------- | ------------------------------------------------------------------------------- |
| `cargo fmt --check`                            | Rust formatting matches `rustfmt`.                                              |
| `cargo clippy --all-targets -- -D warnings`    | Library, binary, test, and example targets compile without warnings.            |
| `cargo test`                                   | Unit tests, integration tests, and doc-tests pass.                              |
| `git diff --check`                             | The pending working-tree diff has no whitespace errors or conflict markers.     |

---

## Contract and Behavior

### 1. `cargo fmt --check`

On success, the command exits `0` and prints nothing. On failure, it exits `1` and prints a unified diff for each file that needs formatting, starting with `Diff in <path>:<line>:` and marking removed lines with `-` and replacement lines with `+`. Run `cargo fmt` to apply the formatting, then rerun the check.

### 2. `cargo clippy --all-targets -- -D warnings`

On success, the command exits `0` and ends with ``Finished `dev` profile [unoptimized + debuginfo] target(s) in <duration>``. A cold run prints `Compiling` and `Checking` lines before that line; a warm run prints only the `Finished` line. On failure, it exits `101`; each diagnostic is an `error:` block because `-D warnings` promotes warnings, and the run ends with ``error: could not compile `excalidraw-spatial` (<target>) due to <n> previous errors``, where `<target>` names the failing target, such as `lib` or `lib test`.

### 3. `cargo test`

On success, the command exits `0`. Each test binary prints `running <n> tests`, one `test <name> ... ok` line per test, then `test result: ok. <n> passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in <duration>`. The run covers unit tests in `src/lib.rs` and `src/main.rs`, every integration binary in `tests/`, and doc-tests; doc-tests report `running 0 tests`. On failure, the command exits `101`. The failing binary prints a `failures:` block with the test name and panic message, then `test result: FAILED. <p> passed; <f> failed; 0 ignored; 0 measured; 0 filtered out; finished in <duration>`, and cargo ends with ``error: test failed, to rerun pass `--test <binary>``.

Integration tests that spawn the `excal` binary set `EXCAL_FIXED_NOW_MS=1700000000000` on the child process. The value pins element `created` and `updated` timestamps, so generated JSON is byte-identical across runs. `cargo test` needs no environment setup; the variable's CLI contract is in [`docs/cli-reference.md`](cli-reference.md#1-exit-codes-and-environment-variables).

### 4. `git diff --check`

On success, the command exits `0` and prints nothing. On failure, it exits `2` and prints one line per violation, such as `README.md:213: trailing whitespace.` followed by the offending line. The command inspects the unstaged diff between the index and the working tree; it does not inspect committed content or untracked files.

---

## Failures and Limits

- Cold runs compile the crate and its dependencies, so the first clippy or test run takes longer and prints build progress; duration values vary between runs.
- After a test binary fails, cargo reports the failure and does not run the remaining test binaries. Rerun a single suite with `cargo test --test <binary>`.
- The repository has no pull-request check workflow. These commands are the verification surface; the release workflow builds tagged binaries without running the suite.

---

## Further Exploration

- [`docs/cli-reference.md`](cli-reference.md): subcommand syntax, exit codes, output contracts, and the `EXCAL_FIXED_NOW_MS` variable.
- [`docs/architecture.md`](architecture.md): scene-graph reconstruction rules, spatial algorithms, and JSON mutation invariants exercised by the tests.
- [`docs/authoring-tutorial.md`](authoring-tutorial.md): end-to-end authoring workflow whose final step verifies the result with `excal lint --strict`.

---

## Dev Note

None.

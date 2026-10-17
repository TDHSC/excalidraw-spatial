# CLI Command and Output Reference (`excal`)

## Summary

This reference defines the command-line syntax, flags, aliases, exit codes, environment variables, and `stdout`/`stderr` output contracts of the `excal` binary (`src/lib.rs`, `src/main.rs`). AI agents, CI scripts, and engineers use this page to look up exact subcommand invocations across `excal inspect`, `excal lint`, `excal layout`, and `excal draw`. For internal scene-graph reconstruction rules and layout algorithms, see [`docs/architecture.md`](architecture.md); for a guided end-to-end walkthrough, see [`docs/authoring-tutorial.md`](authoring-tutorial.md).

## Table of Contents

- [Overview](#overview)
- [Contract and Behavior](#contract-and-behavior)
- [Failures and Limits](#failures-and-limits)
- [Further Exploration](#further-exploration)
- [Dev Note](#dev-note)

---

## Overview

The `excal` CLI reads a `.excalidraw` file (or `-` for `stdin`) and dispatches to one of four top-level subcommands:

```text
excal inspect <FILE> [--format <ai|json|ascii>]
excal lint    <FILE> [--strict] [--format <text|json>]
excal layout  <SUBCOMMAND> <FILE> [FLAGS] [--in-place | -o <OUT>]
excal draw    <SUBCOMMAND> <FILE> [FLAGS] [--in-place | -o <OUT>]
```

- **Read-only commands (`inspect`, `lint`)**: write the formatted report to `stdout` and never modify `<FILE>`.
- **Mutating commands (`layout`, `draw`)**: write the updated `.excalidraw` JSON to `stdout` by default, to `<OUT>` when `-o <OUT>` is passed, or back to `<FILE>` when `--in-place` is passed (for `draw init <FILE>`, omitting `-o` writes directly to `<FILE>` when `<FILE> != "-"`). Action and post-change lint summaries are always written to `stderr`.

---

## Contract and Behavior

Each subcommand enforces strict flag combinations, coordinate conventions, and output formats.

### 1. Exit Codes and Environment Variables

| Exit Code / Variable | Value / Condition                                                                                            |
| -------------------- | ------------------------------------------------------------------------------------------------------------ |
| `0`                  | Command succeeded (`lint` has `errors == 0`, and `warnings == 0` when `--strict` is set).                    |
| `1`                  | `excal lint` found at least one `ERROR`, or at least one `WARN` when `--strict` is set.                      |
| `2`                  | Invalid CLI arguments, missing/malformed input file, unresolved target ID, or violated precondition.         |
| `EXCAL_FIXED_NOW_MS` | Optional integer timestamp in milliseconds (`i64`) used instead of the system clock for `created`/`updated`. |

### 2. `excal inspect`

```text
excal inspect <FILE> [--format <ai|json|ascii>]
```

- `--format ai` (default): prints `# excal inspect <file>` followed by the canvas summary line and seven sections (`## Containers`, `## Components`, `## Rows`, `## Columns`, `## Connections`, `## Minimap`, `## Issues`). When `<FILE>` is `-`, the header uses `<stdin>` and generated `run:` commands use `<file>`.
- `--format ascii`: prints only the 64x24 ASCII minimap and legend (`(none)` if the canvas has no components).
- `--format json`: prints a pretty-printed JSON object containing `file`, `canvas` (`null` or `{x1, y1, x2, y2}`), `summary` (`components`, `containers`, `connectors`, `errors`, `warnings`, `info`), `containers`, `components`, `tracks`, `arrows`, and `issues`.

### 3. `excal lint`

```text
excal lint <FILE> [--strict] [--format <text|json>]
```

- Evaluates all 9 rules from [`src/linter/`](../src/linter/README.md):

| Rule                    | Severity         | Trigger Condition                                                                                            |
| ----------------------- | ---------------- | ------------------------------------------------------------------------------------------------------------ |
| `OVERLAP_COMPONENT`     | `ERROR`          | Two non-`Text` sibling components or sibling zones overlap by `> 0.5px` on both axes.                        |
| `CONTAINER_OVERFLOW`    | `ERROR` / `WARN` | Direct child extends outside its container (`ERROR`) or has `< 10px` inner padding (`WARN`).                 |
| `ARROW_CROSSING_NODE`   | `ERROR`          | An `arrow` segment intersects a non-endpoint component's `rect.inflate(-2.0)`.                               |
| `ARROW_LABEL_COLLISION` | `ERROR`          | An arrow's bound label overlaps a component, zone border (`inflate(-4.0)`), or lower-z arrow label.          |
| `TEXT_OVERFLOW`         | `WARN`           | A shape's label exceeds `usable_width - 20px` or `usable_height - 16px`.                                     |
| `DANGLING_ARROW`        | `WARN` / `INFO`  | An arrow endpoint is `Free` or `> 12px` from its bound element (`WARN`), or `Near` without binding (`INFO`). |
| `NEAR_MISS_ALIGNMENT`   | `WARN`           | Adjacent row/column or cross-swimlane components are misaligned by `1..=8px`.                                |
| `UNEVEN_SPACING`        | `INFO`           | A `>= 3` track has `max_gap - min_gap > 20px`, or any adjacent track pair has a positive gap `< 30px`.       |
| `UNBOUND_TEXT`          | `INFO`           | An unlabeled shape contains a single unbound free-text element (`containerId == null`).                      |

- `--format text` (default): prints each issue block (`{SEVERITY:<5} {RULE} {message}`, `fix: ...`, optional `run: ...`) followed by `errors: E, warnings: W, info: I`.
- `--format json`: prints `{ "file": ..., "summary": { "errors": E, "warnings": W, "info": I }, "issues": [...] }`.

### 4. `excal layout`

All `excal layout` subcommands accept `--in-place` or `-o <OUT>` (mutually exclusive) and negative numbers on numeric flags:

| Subcommand      | Syntax and Flags                                                                                                                                                                                                    |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `move`          | `excal layout move <FILE> --id <ID> [--dx <N>] [--dy <N>] [--ripple] [--in-place \| -o <OUT>]` (`--dx` and `--dy` default to `0.0`).                                                                                |
| `align`         | `excal layout align <FILE> --ids <ID,ID,...> --axis <left\|center\|right\|top\|middle\|bottom> [--in-place \| -o <OUT>]` (`>= 2` IDs).                                                                              |
| `distribute`    | `excal layout distribute <FILE> --ids <ID,ID,...> --direction <horizontal\|vertical> [--gap <N>] [--in-place \| -o <OUT>]`.                                                                                         |
| `place`         | `excal layout place <FILE> --id <ID> (--right-of <A> \| --left-of <A> \| --below <A> \| --above <A>) [--gap <N>] [--align <start\|center\|end>] [--in-place \| -o <OUT>]` (defaults: `--gap 60`, `--align center`). |
| `fit-container` | `excal layout fit-container <FILE> --id <ID> [--padding <N>] [--expand-only] [--in-place \| -o <OUT>]` (default `--padding 40`).                                                                                    |
| `fit-text`      | `excal layout fit-text <FILE> (--id <ID> \| --all) [--padding-x <N>] [--padding-y <N>] [--in-place \| -o <OUT>]` (defaults: `24`, `16`).                                                                            |

- **`stderr` on no-op**: when `strip_nops` leaves an empty change, `excal layout` prints `excal: nothing to change` and leaves `<FILE>` untouched.
- **`stderr` on applied change**: prints two lines:

```text
excal: moved M item(s), resized R item(s), updated C connector(s)
excal: lint after change: errors: E, warnings: W, info: I
```

### 5. `excal draw`

`draw node`, `draw zone`, `draw frame`, `draw text`, and `draw batch` automatically initialize an empty document in memory when `<FILE>` does not exist on disk (`<FILE> != "-"`). All text flags normalize literal `\n` into a newline character.

| Subcommand (Aliases)      | Syntax and Key Flags                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `init`                    | `excal draw init <FILE> [--force] [--in-place \| -o <OUT>]`                                                                                                                                                                                                                                                                                                                                                                                 |
| `node`                    | `excal draw node <FILE> --id <ID> [--label\|--text <TEXT>] [--shape <rectangle\|ellipse\|diamond\|stickynote>] [--x <N> --y <N> \| --right-of <A> \| --left-of <A> \| --below <A> \| --above <A>] [--gap <N>] [--align <start\|center\|end>] [--in <CONTAINER>] [--from <ID> [--edge-label <TEXT>] [--elbow]] [--width <N>] [--height <N>] [--bg <HEX>] [--stroke <HEX>] [--stroke-style <solid\|dashed\|dotted>] [--in-place \| -o <OUT>]` |
| `connect` (`arrow`)       | `excal draw connect <FILE> --from <A> --to <B> [--id <ID>] [--label\|--text <TEXT>] [--elbow] [--via <x=N\|y=N>] [--from-side <left\|right\|top\|bottom>] [--to-side <left\|right\|top\|bottom>] [--in-place \| -o <OUT>]`                                                                                                                                                                                                                  |
| `route` (`reroute`)       | `excal draw route <FILE> --id <ARROW_ID> --via <x=N\|y=N> [--in-place \| -o <OUT>]`                                                                                                                                                                                                                                                                                                                                                         |
| `zone`                    | `excal draw zone <FILE> --id <ID> --title\|--label\|--text <TEXT> (--children <ID,ID,...> [--padding <N>] \| --x <N> --y <N> --width <N> --height <N>) [--in <CONTAINER>] [--bg <HEX>] [--stroke <HEX>] [--stroke-style <solid\|dashed\|dotted>] [--in-place \| -o <OUT>]`                                                                                                                                                                  |
| `frame`                   | `excal draw frame <FILE> --id <ID> [--name\|--label\|--title <TEXT>] (--children <ID,ID,...> [--padding <N>] \| --x <N> --y <N> --width <N> --height <N>) [--in-place \| -o <OUT>]`                                                                                                                                                                                                                                                         |
| `text`                    | `excal draw text <FILE> --id <ID> --text\|--label <TEXT> [--x <N> --y <N> \| --right-of <A> \| --left-of <A> \| --below <A> \| --above <A>] [--gap <N>] [--align <start\|center\|end>] [--font-size <N>] [--in-place \| -o <OUT>]`                                                                                                                                                                                                          |
| `label`                   | `excal draw label <FILE> --id <TARGET_ID> --text\|--label <TEXT> [--in-place \| -o <OUT>]`                                                                                                                                                                                                                                                                                                                                                  |
| `remove` (`rm`, `delete`) | `excal draw remove <FILE> --id <TARGET_ID> [--keep-arrows] [--in-place \| -o <OUT>]`                                                                                                                                                                                                                                                                                                                                                        |
| `batch`                   | `excal draw batch <FILE> [--script <PATH>] [--in-place \| -o <OUT>]` (reads script from `stdin` when `--script` is omitted or `-`).                                                                                                                                                                                                                                                                                                         |

- **`stderr` contract**: every `excal draw` command prints:

```text
excal: added A item(s), updated U item(s), removed R item(s)
excal: lint after change: errors: E, warnings: W, info: I
```

When `E > 0 || W > 0 || I > 0`, `excal draw` immediately appends the full multi-line lint blocks (`SEVERITY RULE ...`, `fix: ...`, `run: ...`) to `stderr` while still exiting with code `0`.

---

## Failures and Limits

All runtime and validation errors print `excal: error: <message>` to `stderr` and exit with code `2`:

- Passing `--in-place` when `<FILE>` is `-` (`cannot use --in-place with stdin`).
- Passing both `<FILE> == "-"` and `--script -` (or omitting `--script`) on `draw batch` (`cannot read both document and batch script from stdin; pass --script <PATH>`).
- Running `draw init <FILE>` on an existing file without `--force` (`file "<FILE>" already exists; pass --force to overwrite`).
- Creating an element with an empty `--id`, an `--id` starting with `group:`, an `--id` containing whitespace or commas, or an `--id` that already exists in the document.
- Calling `layout distribute` with `< 2` IDs, or `< 3` IDs when `--gap` is omitted.
- Calling `layout align` with `< 2` IDs, or `layout place` with `--id <A>` equal to the anchor `<A>`.
- Failing on line `N` inside `draw batch`: prints `excal: error: line N: <message>` and aborts atomically without writing `<FILE>`.

---

## Further Exploration

- [`docs/authoring-tutorial.md`](authoring-tutorial.md): step-by-step tutorial demonstrating `excal draw`, `excal inspect`, `excal lint`, and `excal layout`.
- [`docs/architecture.md`](architecture.md): internal data structures, scene-graph rules, and JSON patching invariants.
- [`src/author/README.md`](../src/author/README.md): authoring defaults, port spreading, and batch script tokenization.
- [`src/mutator/README.md`](../src/mutator/README.md): layout transformation rules and container membership retention.
- [`src/linter/README.md`](../src/linter/README.md): lint thresholds and fix command synthesis.

---

## Dev Note

None.

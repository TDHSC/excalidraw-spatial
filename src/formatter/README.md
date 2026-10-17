<!-- markdownlint-disable MD033 -->

# Output Formatter (`src/formatter/`)

## Summary

The `formatter` module serializes a reconstructed `Scene` and its `[LintIssue]` slice into human/AI text reports, 64x24 ASCII minimaps, and structured JSON documents. Call `formatter::ai_report::format_inspect` for the full markdown spatial report (`excal inspect --format ai`), `formatter::ascii_map::render` for the standalone character grid (`--format ascii`), `formatter::format_inspect_json` for machine-readable scene and lint JSON (`--format json`), or `formatter::format_lint_text` / `format_lint_json` for `excal lint` and `excal draw` diagnostics.

## Table of Contents

- [Use This Module](#use-this-module)
- [Understand the Implementation](#understand-the-implementation)
- [Further Exploration](#further-exploration)
- [Known Limitations and Deferred Work](#known-limitations-and-deferred-work)
- [Dev Note](#dev-note)

---

## Use This Module

Invoke `excal inspect` or `excal lint` on the CLI, or call the formatting functions in `src/formatter/` directly from Rust.

### When to Choose It

Choose `formatter` whenever presenting `Scene` geometry or `LintIssue` diagnostics to an LLM agent, terminal user, or JSON consumer. To compute the underlying scene or lint issues first, call [`src/scene_graph.rs`](../../docs/architecture.md) (`Scene::build`) and [`src/linter/`](../linter/README.md) (`linter::run_lint`).

### Minimal Usage

Render the 64x24 ASCII minimap of a diagram from the CLI:

```sh
cargo run --quiet -- inspect tests/fixtures/basic_flow.excalidraw --format ascii
```

Observable output on `stdout`:

```text
AAAAAAAAAAAAAAAA        BBBBBBBBBBBBBBBB        CCCCCCCCCCCCCCCC
AAAAAAAAAAAAAAAA********BBBBBBBBBBBBBBBB********CCCCCCCCCCCCCCCC
AAAAAAAAAAAAAAAA        BBBBBBBBBBBBBBBB        CCCCCCCCCCCCCCCC
AAAAAAAAAAAAAAAA        BBBBBBBBBBBBBBBB        CCCCCCCCCCCCCCCC
A=client B=api C=svc
```

From Rust, format both the AI inspect report and lint summary:

```rust
use excalidraw_spatial::{formatter, linter, scene_graph::Scene, schema::Document};

let doc = Document::load("tests/fixtures/basic_flow.excalidraw")?;
let scene = Scene::build(&doc);
let issues = linter::run_lint(&scene, "tests/fixtures/basic_flow.excalidraw");
let report = formatter::ai_report::format_inspect(&scene, &issues, "tests/fixtures/basic_flow.excalidraw");
assert!(report.starts_with("# excal inspect tests/fixtures/basic_flow.excalidraw\n"));
```

| Function                    | CLI Equivalent                        | Output Contract                                                                                                  |
| --------------------------- | ------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `ai_report::format_inspect` | `excal inspect <FILE> --format ai`    | Markdown report with `Containers`, `Components`, `Rows`, `Columns`, `Connections`, `Minimap`, `Issues`.          |
| `ascii_map::render`         | `excal inspect <FILE> --format ascii` | Up to 64x24 character grid of containers (`.`), components (`A`-`Z`, `a`-`z`, `0`-`9`, `#`), and arrows (`*`).   |
| `format_inspect_json`       | `excal inspect <FILE> --format json`  | Pretty-printed JSON with `file`, `canvas`, `summary`, `containers`, `components`, `tracks`, `arrows`, `issues`.  |
| `format_lint_text`          | `excal lint <FILE> --format text`     | Multi-line issue blocks (`SEVERITY RULE message`, `fix:`, `run:`) followed by `errors: E, warnings: W, info: I`. |
| `format_lint_issues_only`   | `excal draw ...` (`stderr` lines 3+)  | Same multi-line issue blocks without the trailing summary line (empty string when `issues` is empty).            |
| `format_lint_json`          | `excal lint <FILE> --format json`     | Pretty-printed JSON with `file`, `summary` (`errors`, `warnings`, `info`), and `issues`.                         |

---

## Understand the Implementation

<details>
<summary>Implementation internals — click to expand</summary>

`formatter` combines spatial queries from [`src/spatial.rs`](../../docs/architecture.md) with deterministic rounding (`geometry::fmt_px`, `fmt_rect`, `fmt_point`) so every report is reproducible across platforms.

1. **AI Spatial Report (`ai_report.rs`)**:
    - Displays the canvas union bounds across all containers, components, and connectors (or `canvas empty` when all three are empty) alongside object and issue counts.
    - `Containers`: lists each container's ID, kind (`zone` or `frame`), single-line `inline_text` name, `Rect`, parent ID (`-` when `None`), 4-side inner padding (`T.. R.. B.. L..` or `padding -` when empty), and direct child IDs (`[c1, c2, ...]`).
    - `Components`: lists each component's ID, single-line `inline_text` label (without 30-char truncation), kind (`rectangle`, `ellipse`, `diamond`, `group`, `text`, etc.), `Rect`, center `c(x,y)`, parent ID, and—for non-`Text` components—4-way corridor neighbors (`L=... R=... T=... B=...` with pixel gap).
    - `Rows` and `Columns`: groups components by `parent` (`top` first, then containers in ascending `z` order), splits tracks around sibling containers (`spatial::rows_with_obstacles`, `columns_with_obstacles`), sorts tracks by `pos`, and prints `id1 --gap-- id2`.
    - `Connections`: prints each connector's ID, start/end endpoint formatting (`id`, `~id` for `Near`, or `(x,y)` for `Free`), optional label, routing style (`elbow` when `elbowed`, `straight` when 2 points, `polyline` when `>= 3` points), and vertex coordinates.
2. **64x24 ASCII Minimap (`ascii_map.rs`)**:
    - Returns `"(none)"` when `scene.components` is empty.
    - Computes the union bounding box `u` of all containers, components, and connectors and scales it into a grid of width `cols = 64` and height `rows = clamp(round(u.height / max(u.width, 1) * 32), 4, 24)`.
    - Draws container borders with `'.'` first, connector segments with `'*'` second (sampling `max(|dx|, |dy|) * 2` steps per segment), and filled component boxes third using symbols `'A'..='Z'`, `'a'..='z'`, `'0'..='9'`, and `'#'` (for index `>= 62`), followed by a legend line `A=id1 B=id2 ...`.
3. **JSON Serialization (`mod.rs`)**:
    - Serializes `Rect` and `Point` values rounded to integers (`round_i64`) while preserving exact raw strings (including embedded newlines) in `label` and `name` fields.

| File                           | Responsibility                                                                                |
| ------------------------------ | --------------------------------------------------------------------------------------------- |
| [`mod.rs`](mod.rs)             | `format_lint_text`, `format_lint_issues_only`, `format_lint_json`, and `format_inspect_json`. |
| [`ai_report.rs`](ai_report.rs) | Human/AI markdown report generator (`# excal inspect <file>`).                                |
| [`ascii_map.rs`](ascii_map.rs) | 64x24 character-grid rasterizer for containers, connectors, and components.                   |

</details>

---

## Further Exploration

- [`docs/cli-reference.md`](../../docs/cli-reference.md): complete schema and examples for `excal inspect` and `excal lint` output formats.
- [`docs/architecture.md`](../../docs/architecture.md): how `scene_graph` and `spatial` compute the tracks, neighbors, and attachments rendered by `formatter`.
- [`src/linter/README.md`](../linter/README.md): severity ordering and message formatting for `LintIssue` records.

---

## Known Limitations and Deferred Work

Callers should note these formatting rules:

- `ai_report` collapses newlines in component and container labels via `linter::inline_text` so every bullet stays on one line, whereas `format_inspect_json` preserves literal `\n` characters in JSON strings.
- `ascii_map::render` assigns `'#'` to every component past the 62nd component (`A-Z`, `a-z`, `0-9`).

### Dev Note

<details>
<summary>Working context for maintainers — click to expand</summary>

None.

</details>

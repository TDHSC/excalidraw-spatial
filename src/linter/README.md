<!-- markdownlint-disable MD033 -->

# Spatial Linter (`src/linter/`)

## Summary

The `linter` module detects geometric and structural defects in a reconstructed `Scene` and produces sorted `LintIssue` records with pixel-exact explanations and executable `excal layout` repair commands. Call `linter::run_lint(&scene, file)` after building a `Scene` to validate component overlaps, container margins, connector crossings, arrow label collisions, text overflow, dangling endpoints, near-miss alignments, uneven track spacing, and unbound inner text. It is a pure read-only analysis pass that never mutates the document.

## Table of Contents

- [Use This Module](#use-this-module)
- [Understand the Implementation](#understand-the-implementation)
- [Further Exploration](#further-exploration)
- [Known Limitations and Deferred Work](#known-limitations-and-deferred-work)
- [Dev Note](#dev-note)

---

## Use This Module

Call `linter::run_lint(&scene, file)` from Rust or run `excal lint <FILE>` from the command line to evaluate all nine rules against a diagram.

### When to Choose It

Use `linter` when you need deterministic geometric diagnostics (`Error`, `Warn`, `Info`) and suggested `excal layout` commands without modifying the input `.excalidraw` file. When you also need the full inventory of containers, components, tracks, and connections alongside the lint issues, use [`src/formatter/`](../formatter/README.md) (`excal inspect`). When you want to apply a fix command to the document, use [`src/mutator/`](../mutator/README.md) (`excal layout`).

### Minimal Usage

Run `excal lint` against a fixture with an overlapping pair of components:

```sh
cargo run --quiet -- lint tests/fixtures/overlap.excalidraw
```

Observable output on `stdout` (exits with code `1` because an `ERROR` is present):

```text
ERROR OVERLAP_COMPONENT api "API" overlaps auth "Auth" by 24x60px
      fix: move auth right by 64px to leave a 40px gap
      run: excal layout move tests/fixtures/overlap.excalidraw --id auth --dx 64
errors: 1, warnings: 0, info: 0
```

From Rust, invoke `run_lint` on a `Scene`:

```rust
use excalidraw_spatial::{linter, scene_graph::Scene, schema::Document};

let doc = Document::load("tests/fixtures/overlap.excalidraw")?;
let scene = Scene::build(&doc);
let issues = linter::run_lint(&scene, "tests/fixtures/overlap.excalidraw");
assert_eq!(issues[0].rule, "OVERLAP_COMPONENT");
```

| Constant / Parameter        | Default | Meaning                                                                                      |
| --------------------------- | ------- | -------------------------------------------------------------------------------------------- |
| `file`                      | —       | File path interpolated into generated `run: excal layout ...` commands (`<file>` for stdin). |
| `MIN_GAP`                   | `40`    | Target clearance in pixels added when resolving overlaps or arrow label collisions.          |
| `NEAR_DIST`                 | `12`    | Maximum distance in pixels for an unbound arrow endpoint to count as `Attach::Near`.         |
| `CONTAINER_WARN_PADDING`    | `10`    | Minimum inner padding in pixels before `CONTAINER_OVERFLOW` emits a `Warn`.                  |
| `TEXT_PAD_X` / `TEXT_PAD_Y` | `20/16` | Horizontal and vertical padding subtracted from usable shape size in `TEXT_OVERFLOW`.        |
| `NEAR_MISS_MAX`             | `8`     | Maximum axis delta in pixels (`1..=8px`) reported by `NEAR_MISS_ALIGNMENT`.                  |
| `UNEVEN_DIFF` / `TIGHT_GAP` | `20/30` | Gap spread (`> 20px`) or tight gap (`< 30px`) thresholds for `UNEVEN_SPACING`.               |

---

## Understand the Implementation

<details>
<summary>Implementation internals — click to expand</summary>

`run_lint` executes four rule groups over `&Scene` and sorts the resulting `Vec<LintIssue>` by severity (`Error`, `Warn`, `Info`), rule name (lexicographical), and first target ID (`targets[0]`).

1. **Overlap and Container Bounds (`overlap.rs`)**:
    - `OVERLAP_COMPONENT` (`Error`): checks non-`Text` sibling components (`a.parent == b.parent`) and sibling zones in z-order. It computes horizontal and vertical push distances to leave a `MIN_GAP` (`40px`) clearance, picks the axis aligned with any connecting arrow or the smaller push distance, and appends `--ripple` when downstream siblings exist along the push vector.
    - `CONTAINER_OVERFLOW`: checks each container against its direct children and direct child zones. It emits `Error` (`fit-container --id <C> --padding 40`) when a child protrudes outside the container (or into a zone's top title band) and `Warn` (`--expand-only`) when the minimum padding falls in `0..10px`.
2. **Connector Geometry (`connectors.rs`)**:
    - `ARROW_CROSSING_NODE` (`Error`): clips each `arrow` segment against `node.rect.inflate(-2.0)` for every non-endpoint component, suggesting how far to move the node above/below or left/right of the arrow.
    - `ARROW_LABEL_COLLISION` (`Error`): checks each arrow's `label_rect` against components, zone borders (`inflate(-4.0)`), and lower-z arrow labels. When the collided box is the arrow's own endpoint, it emits an `excal layout move` command to lengthen the arrow (with `--ripple` if downstream siblings exist).
    - `DANGLING_ARROW`: emits `Warn` when a `Free` endpoint lies past `NEAR_DIST` (`12px`) or a `Bound` element lies past `12px`, and `Info` when a `Near` endpoint has `containerId == None` within `12px`.
3. **Text and Track Alignment (`alignment.rs`)**:
    - `TEXT_OVERFLOW` (`Warn`): compares a shape's label size (`text_metrics::text_size`) against `rect * text_factor(kind) - (TEXT_PAD_X, TEXT_PAD_Y)` and suggests `excal layout fit-text`.
    - `NEAR_MISS_ALIGNMENT` (`Warn`): checks adjacent pairs in horizontal rows (`top`, `middle`, `bottom`), adjacent pairs in vertical columns (`left`, `center`, `right`), and cross-container pairs in parallel swimlanes, reporting minimum non-zero axis deltas in `[0.5, 8.0]px` with `excal layout align`.
    - `UNEVEN_SPACING` (`Info`): inspects obstacle-split rows and columns (`spatial::rows_with_obstacles`, `columns_with_obstacles`). Tracks with `>= 3` members and positive gaps emit `excal layout distribute` when `max_gap - min_gap > 20px`; any positive gap `< 30px` emits a tight-gap notice.
4. **Scene Reconstruction Notes (`mod.rs`)**:
    - Converts `Scene::notes` (`NoteKind::UnboundText`) into `INFO UNBOUND_TEXT` issues when an unlabeled shape absorbs a single inner free-text element.

| File                             | Responsibility                                                                                       |
| -------------------------------- | ---------------------------------------------------------------------------------------------------- |
| [`mod.rs`](mod.rs)               | Shared constants, `Severity` and `LintIssue` types, `inline_text`/`label_str` helpers, `run_lint`.   |
| [`overlap.rs`](overlap.rs)       | `OVERLAP_COMPONENT` and `CONTAINER_OVERFLOW` detection and fix synthesis.                            |
| [`connectors.rs`](connectors.rs) | `ARROW_CROSSING_NODE`, `ARROW_LABEL_COLLISION`, and `DANGLING_ARROW` checks.                         |
| [`alignment.rs`](alignment.rs)   | `TEXT_OVERFLOW`, `NEAR_MISS_ALIGNMENT` (intra-track and cross-swimlane), and `UNEVEN_SPACING` rules. |

</details>

---

## Further Exploration

- [`docs/cli-reference.md`](../../docs/cli-reference.md): CLI flags (`--strict`, `--format json`) and exit code contract for `excal lint`.
- [`docs/architecture.md`](../../docs/architecture.md): how `scene_graph` and `spatial` compute parent hierarchies and obstacle-split tracks consumed by `linter`.
- [`src/mutator/README.md`](../mutator/README.md): how `excal layout` executes the `run:` commands emitted by lint rules.
- [`src/formatter/README.md`](../formatter/README.md): how lint issues are formatted into text, JSON, and `excal inspect` reports.

---

## Known Limitations and Deferred Work

The following geometric constraints apply to all lint rules:

- Element rotation (`angle`) is ignored; all bounding boxes are evaluated as axis-aligned rectangles.
- Curved arrows are evaluated as piecewise linear polylines through their stored `points` array.
- `INFO UNBOUND_TEXT` and `WARN/INFO DANGLING_ARROW` provide `fix:` prose instructions (`command: None`) because binding a free text element or re-attaching a dangling endpoint requires authoring intent rather than a pure layout shift.

### Dev Note

<details>
<summary>Working context for maintainers — click to expand</summary>

None.

</details>

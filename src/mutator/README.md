<!-- markdownlint-disable MD033 -->

# Layout Mutator (`src/mutator/`)

## Summary

The `mutator` module computes and applies relative geometric layout transformations (`move`, `align`, `distribute`, `place`, `fit-container`, and `fit-text`) to `.excalidraw` documents. Given a `LayoutCommands` variant and a reconstructed `Scene`, `mutator::layout_change` produces a `Change { moves, resizes }` plan that automatically retains parent zone membership, pushes overlapping sibling zones, and co-translates orthogonal column stacks. Calling `mutator::apply` patches the raw JSON elements in place, re-centers bound labels, and re-clips or re-routes attached connectors without altering unrelated element properties.

## Table of Contents

- [Use This Module](#use-this-module)
- [Understand the Implementation](#understand-the-implementation)
- [Further Exploration](#further-exploration)
- [Known Limitations and Deferred Work](#known-limitations-and-deferred-work)
- [Dev Note](#dev-note)

---

## Use This Module

Use `excal layout <SUBCOMMAND>` from the CLI or call `mutator::layout_change`, `mutator::strip_nops`, and `mutator::apply` in Rust.

### When to Choose It

Choose `mutator` when repositioning, aligning, spacing, or resizing existing components and containers while keeping their bound text and connectors synchronized. When you need to create new shapes, containers, connectors, or relabel/delete existing objects, use [`src/author/`](../author/README.md) (`excal draw`). When you only need to diagnose layout issues without modifying the document, use [`src/linter/`](../linter/README.md) (`excal lint`).

### Minimal Usage

Move an overlapping component on the command line and inspect the two-line `stderr` summary:

```sh
cargo run --quiet -- layout move tests/fixtures/overlap.excalidraw --id auth --dx 64 -o /tmp/overlap_fixed.excalidraw
```

Observable output on `stderr`:

```text
excal: moved 1 item(s), resized 0 item(s), updated 0 connector(s)
excal: lint after change: errors: 0, warnings: 0, info: 0
```

From Rust, compute and apply a layout change:

```rust
use excalidraw_spatial::{lib::LayoutCommands, mutator, scene_graph::Scene, schema::Document};

let mut doc = Document::load("tests/fixtures/overlap.excalidraw")?;
let scene = Scene::build(&doc);
let mut change = mutator::ops::move_target(&scene, "auth", 64.0, 0.0, false)?;
mutator::strip_nops(&mut change, &scene);
let report = mutator::apply(&mut doc, &scene, &change);
doc.finish(1_700_000_000_000);
assert_eq!(report.moved, 1);
```

| Subcommand / Operation | Key Parameters                                                                                       | Behavior                                                                                              |
| ---------------------- | ---------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `move`                 | `--id <ID> [--dx <N>] [--dy <N>] [--ripple]`                                                         | Translates target (and descendants if container); `--ripple` co-translates downstream band siblings.  |
| `align`                | `--ids <ID,ID,...> --axis <left\|center\|right\|top\|middle\|bottom>`                                | Aligns all listed targets to the first listed target (`ids[0]`) along the chosen axis.                |
| `distribute`           | `--ids <ID,ID,...> --direction <horizontal\|vertical> [--gap <N>]`                                   | Spaces `>= 2` targets evenly by `--gap` or across their outer span (`>= 3` required without `--gap`). |
| `place`                | `--id <ID> (--right-of\|--left-of\|--below\|--above <A>) [--gap <N>] [--align <start\|center\|end>]` | Places `<ID>` relative to anchor `<A>` (default `--gap 60`, `--align center`).                        |
| `fit-container`        | `--id <ID> [--padding <N>] [--expand-only]`                                                          | Resizes a `Zone` or `Frame` around its children with `--padding` (default `40`).                      |
| `fit-text`             | `(--id <ID> \| --all) [--padding-x <N>] [--padding-y <N>]`                                           | Grows `TEXT_SHAPES` rectangles/ellipses/diamonds so labels fit (default padding `24`, `16`).          |

---

## Understand the Implementation

<details>
<summary>Implementation internals — click to expand</summary>

`mutator` separates layout planning (`ops.rs`) from in-place JSON patching and connector synchronization (`mod.rs`, `sync_bindings.rs`).

1. **Planning & Zone Membership Retention (`ops.rs`)**:
    - Every operation resolves user queries via `Scene::resolve` and builds a `Change { moves, resizes }`.
    - `distribute` calls `cotranslate_orthogonal_stacks`, which shifts orthogonal column/row partners that share the same cross-axis track and are connected by a single-hop vertical/horizontal connector (such as scatter-gather shards) alongside the distributed anchors.
    - `retain_zone_membership` prevents layout moves from accidentally ejecting children from their parent container or causing sibling container collisions:
        - **Pass 1 (`grow_membership`)**: under `RetainPolicy::Arrange` (`align`, `distribute`, `move --ripple`), a child that moves outside its parent zone or closer than `MIN_GAP` (`40px`) to a `Zone` or `Frame` edge expands that side of its parent container, propagating up the ancestor chain. Under `RetainPolicy::Position` (`move`, `place`), straddling moves and same-parent `place` operations expand the parent container, while full exits (`move` completely outside or `place` relative to an external anchor) release the child from the zone.
        - **Pass 2 (`push_overlapping_sibling_zones`)**: when an expanded zone overlaps a disjoint sibling zone (or claims a sibling zone's child), it pushes the sibling zone and same-band downstream siblings outward to leave a `MIN_GAP` (`40px`) margin, then runs `grow_membership` once more so any enclosing parent zone absorbs the pushed sibling.
2. **In-Place Element Patching (`mod.rs`)**:
    - `strip_nops` removes zero-delta moves (`<= 1e-9`) and identical resizes so no-op commands leave the document untouched.
    - `apply` expands container moves to all descendant components, nested containers, and connectors whose both endpoints move by the same `(dx, dy)` vector.
    - Resized shapes re-center their bound text (`bound_text_xy`) and materialize estimated text dimensions when stored `width <= 0`. Resized zones keep top-aligned bound titles at `(x1 + 5, y1 + 5)` and clamp unbound titles above child components.
3. **Connector Synchronization (`sync_bindings.rs`)**:
    - `sync_arrows` updates any connector whose `Bound` or `Near` endpoint moved or resized (skipping connectors already translated as a whole inside a moved container).
    - **2-point straight arrows**: computes focus points from `fixedPoint` (preserving parallel multi-arrow bundle spacing when `fixedPoint` sits at `0.5`) and clips the segment against `s_rect.inflate(gs)` and `e_rect.inflate(ge)` via `geometry::clip_segment` (falling back to uninflated rects or keeping a `4px` minimum forward segment when boxes overlap).
    - **2-point elbow arrows**: computes a 4-point orthogonal route (`elbow_two_point_route`) when endpoints are offset on both axes, choosing the vertical or horizontal leg that avoids crossing endpoint boxes.
    - **3+ point polylines**: shifts moved endpoints along their side normal and adjusts the adjacent interior segment to preserve orthogonality (or collapses a 3-point collinear elbow into 2 points). Finally, bound arrow labels are repositioned at `polyline_point_at(&abs, label_position)`.

| File                                   | Responsibility                                                                                       |
| -------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| [`mod.rs`](mod.rs)                     | `Change`, `ApplyReport`, `layout_change`, `strip_nops`, `apply`, and bound text/title repositioning. |
| [`ops.rs`](ops.rs)                     | The 6 layout operations, orthogonal stack co-translation, and `retain_zone_membership`.              |
| [`sync_bindings.rs`](sync_bindings.rs) | Straight-arrow Liang-Barsky re-clipping, 2-point/multi-point elbow routing, and arrow label sync.    |

</details>

---

## Further Exploration

- [`docs/cli-reference.md`](../../docs/cli-reference.md): complete CLI flags and error conditions for `excal layout`.
- [`docs/authoring-tutorial.md`](../../docs/authoring-tutorial.md): hands-on walkthrough showing how `excal layout` repairs lint issues.
- [`src/author/README.md`](../author/README.md): how `excal draw` reuses `mutator::apply` for container auto-expansion and `fit-text` after relabeling.
- [`src/linter/README.md`](../linter/README.md): how `linter` synthesizes the `excal layout` commands executed by this module.

---

## Known Limitations and Deferred Work

Callers should account for these boundaries when invoking `mutator`:

- `mutator::apply` never modifies `boundElements`, `startBinding`, `endBinding`, `frameId`, `groupIds`, or `fixedSegments`; structural binding changes belong exclusively to [`src/author/`](../author/README.md).
- `fit-text --id <ID>` rejects `Group` components, containers, and connectors with exit code `2`; `fit-text --all` silently skips non-`TEXT_SHAPES` components.
- `fit-container` on a nested zone clamps upward expansion at the parent zone's top title band so the child zone never encroaches on the parent's title.

### Dev Note

<details>
<summary>Working context for maintainers — click to expand</summary>

None.

</details>

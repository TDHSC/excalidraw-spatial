<!-- markdownlint-disable MD033 -->

# Diagram Authoring (`src/author/`)

## Summary

The `author` module builds and edits `.excalidraw` diagrams through high-level semantic operations (`init`, `node`, `connect`, `route`, `zone`, `frame`, `text`, `label`, `remove`, and `batch`). Call `author::apply_draw` or `author::batch::execute_script` to synthesize valid Excalidraw JSON elements with deterministic 31-bit FNV-1a seeds, shape-factor text sizing, relative placement, bidirectional connector bindings, obstacle-aware elbow routing, and automatic container z-ordering and expansion. After every authoring command, `excal draw` runs the geometric linter and prints any actionable `run:` repair commands to `stderr`.

## Table of Contents

- [Use This Module](#use-this-module)
- [Understand the Implementation](#understand-the-implementation)
- [Further Exploration](#further-exploration)
- [Known Limitations and Deferred Work](#known-limitations-and-deferred-work)
- [Dev Note](#dev-note)

---

## Use This Module

Use `excal draw <SUBCOMMAND>` on the command line or call `author::apply_draw` and `author::batch::execute_script` in Rust.

### When to Choose It

Choose `author` when creating a new `.excalidraw` file, adding nodes/containers/connectors/annotations, rerouting arrows through waypoints (`--via`), updating text labels (`label`), deleting objects (`remove`), or executing a multi-line script atomically (`batch`). When you only need to move, align, distribute, or resize existing elements without creating or deleting bindings, use [`src/mutator/`](../mutator/README.md) (`excal layout`).

### Minimal Usage

Create a two-node connected diagram in a single command (auto-initializing the file if it does not exist):

```sh
cargo run --quiet -- draw batch /tmp/demo.excalidraw --in-place <<'EOF'
node --id api --label "API Gateway" --x 100 --y 100
node --id db --label "Orders DB" --right-of api --from api --edge-label "SQL"
EOF
```

Observable output on `stderr`:

```text
excal: added 3 item(s), updated 0 item(s), removed 0 item(s)
excal: lint after change: errors: 0, warnings: 0, info: 0
```

From Rust, execute a batch script against an in-memory `Document`:

```rust
use excalidraw_spatial::{author::batch::execute_script, schema::Document};

let mut doc = Document::empty();
let script = "node --id a --label \"Service A\" --x 100 --y 100\n";
let report = execute_script(&mut doc, script, 1_700_000_000_000)?;
doc.finish(1_700_000_000_000);
assert_eq!(report.added, 1);
```

| Subcommand       | Aliases        | Purpose                                                                                                    |
| ---------------- | -------------- | ---------------------------------------------------------------------------------------------------------- |
| `init <FILE>`    | —              | Initialize an empty `.excalidraw` document (`--force` overwrites an existing file).                        |
| `node <FILE>`    | —              | Create a shape (`rectangle`, `ellipse`, `diamond`, `stickynote`) with optional bound label and `--from`.   |
| `connect <FILE>` | `arrow`        | Connect two components or containers with a bound straight, `--elbow`, or `--via` orthogonal arrow.        |
| `route <FILE>`   | `reroute`      | Reroute an existing bound or near arrow through an orthogonal `--via <x=N\|y=N>` waypoint.                 |
| `zone <FILE>`    | —              | Create a top-titled visual zone rectangle wrapping `--children` or at explicit `--x --y --width --height`. |
| `frame <FILE>`   | —              | Create an Excalidraw `frame` tagging `--children` (`frameId`) or at explicit `--x --y --width --height`.   |
| `text <FILE>`    | —              | Create an unbound free-text annotation at explicit coordinates or relative to an anchor.                   |
| `label <FILE>`   | —              | Set or replace the text of a shape, free-text component, zone title, frame name, or arrow label.           |
| `remove <FILE>`  | `rm`, `delete` | Delete a component, container, or connector and clean up `boundElements` and attached arrows.              |
| `batch <FILE>`   | —              | Execute a script of `draw` and `layout` lines atomically in memory (`--script <PATH>` or `stdin`).         |

---

## Understand the Implementation

<details>
<summary>Implementation internals — click to expand</summary>

`author` constructs raw `serde_json::Value` elements in fixture key order and synchronizes `Document`'s typed views via `doc.refresh()` between composite steps.

1. **Deterministic Identity and Element Synthesis (`mod.rs`)**:
    - `fnv1a_u31(salt, id)` hashes `"seed:{id}"` and `"nonce:{id}"` with 64-bit FNV-1a and maps the digest into `1..2_147_483_647` so every created element gets deterministic `seed` and `versionNonce` values.
    - Newly authored elements set `"index": null` and are recorded in `doc.created` so `doc.finish(now_ms)` keeps their `version == 1` even if subsequent steps in the same command or batch resize or move them.
    - `normalize_text` converts literal two-character `\n` sequences into newline characters before estimating text metrics.
2. **Shape Auto-Sizing, Placement, and Container Cascading (`draw_node`, `draw_zone`, `draw_frame`)**:
    - `draw_node` sizes shapes to `max(160x80, ceil((tw + 48) / f) x ceil((th + 32) / f))` using `text_factor(shape)` (`1.0` for rectangles/stickynotes, `0.707` for ellipses, `0.5` for diamonds).
    - When `--from <ID>` and `--edge-label <TEXT>` are combined with relative placement (`--right-of`, `--left-of`, `--below`, `--above`), `draw_node` clamps the placement gap to at least `ceil(label_span + 24)` (`>= 60px` default) so the new arrow's label never collides with either endpoint.
    - `expand_container_chain` computes the union of existing child rects plus the newly inserted child rect, inflates by `40px` (plus `th + 10px` at the top for titled zones), applies the `0.60` zone child-to-container area guard (`ZONE_CONTAINED_AREA_RATIO = 0.65`), and walks up the parent hierarchy via `mutator::apply`.
    - `draw_zone --children` inserts the zone rectangle and its bound top-left title immediately **before** the lowest-z member element of its children (`min_z`) so children render above the zone background. Conversely, `draw_frame --children` sets `"frameId"` on all descendant elements and inserts the `frame` element immediately **after** the highest-z child (`max_z + 1`).
3. **Connectors, Waypoints, and Port Spreading (`draw_connect`, `draw_route`)**:
    - `connect_horizontal` selects left/right (`h_pair`) or top/bottom (`v_pair`) ports based on axis overlap first and center-delta aspect ratio second. When an `--elbow` arrow connects non-overlapping boxes, `choose_fixed_points` counts intersections against `scene.nodes()` and selects the orientation with fewer node hits.
    - Multiple straight 2-point arrows between the same endpoint pair spread evenly across their shared cross-axis overlap interval (`clamp(0.15, 0.85)`), quantizing `fixedPoint` via `round_json` before clipping.
    - Multiple `--elbow` arrows sharing an auto-selected port side (`Left`, `Right`, `Top`, `Bottom`) spread their `fixedPoint` coordinates across `[0.2, 0.8]` ordered by the opposite endpoint's cross-axis coordinate (`spread_elbow_side`).
4. **Atomic Batch Execution (`batch.rs`)**:
    - `tokenize_line` splits non-comment lines with single- and double-quote awareness (`#` inside or after tokens is kept as literal text).
    - `parse_line` strips an optional leading `excal` token, accepts both `draw <verb>` / `layout <verb>` and bare verbs, rejects nested `batch`, `init`, `--in-place`, and `-o`, and delegates flag parsing to `Cli::try_parse_from`. Any error aborts before writing the output file.

| File                   | Responsibility                                                                                                                   |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| [`mod.rs`](mod.rs)     | Deterministic FNV-1a seeding, JSON element builders, `node`/`zone`/`frame`/`connect`/`route`/`text`/`label`/`remove` execution.  |
| [`batch.rs`](batch.rs) | Quote-aware batch line tokenizer, `clap` command normalization, sequential in-memory execution, and `AuthorReport` accumulation. |

</details>

---

## Further Exploration

- [`docs/authoring-tutorial.md`](../../docs/authoring-tutorial.md): end-to-end tutorial using `excal draw` and `excal draw batch` to construct a system architecture diagram.
- [`docs/cli-reference.md`](../../docs/cli-reference.md): complete flag and alias table for all `excal draw` subcommands.
- [`src/mutator/README.md`](../mutator/README.md): layout mutations and connector re-sync invoked by `author` during container expansion and `label` auto-fitting.
- [`docs/architecture.md`](../../docs/architecture.md): how `Scene::build` recognizes the zones, frames, bound labels, and connectors authored by this module.

---

## Known Limitations and Deferred Work

Keep these constraints in mind when authoring diagrams:

- `draw label` cannot relabel an atomic icon `Group` component (`group:<id>`); it exits with code `2`.
- `draw connect` rejects self-loops (`--from A --to A`) and free `Text` or `Connector` endpoints with exit code `2`.
- `draw route --via` requires an `arrow` connector whose both ends are `Attach::Bound` or `Attach::Near`, and rejects `--via` coordinates that pass through either endpoint's inflated bounding box.

### Dev Note

<details>
<summary>Working context for maintainers — click to expand</summary>

None.

</details>

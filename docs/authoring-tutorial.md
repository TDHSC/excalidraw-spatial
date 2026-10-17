# Author and Repair an Architecture Diagram (`excal`)

## Summary

This tutorial walks through authoring a multi-node `.excalidraw` architecture diagram from scratch using `excal draw`, inspecting its spatial scene graph with `excal inspect`, and resolving geometric defects with `excal lint` and `excal layout`. Follow this guide when learning how `excal` chains relative placement, container wrapping, and closed-loop lint repair. You only need a working Rust toolchain (`cargo`); the final verification step confirms that `excal lint --strict` exits with code `0`.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Steps](#steps)
- [Verify the Result](#verify-the-result)
- [Troubleshooting](#troubleshooting)
- [Further Exploration](#further-exploration)
- [Dev Note](#dev-note)

---

## Prerequisites

- Rust toolchain (`cargo` and `rustc`, 2021 edition) installed.
- Working directory set to the repository root (`/path/to/excalidraw-spatial`).
- Build the binary once before starting:

```sh
cargo build
```

---

## Steps

Each step below creates or modifies `/tmp/arch.excalidraw` using the real `excal` binary. Remove any previous `/tmp/arch.excalidraw` file before starting:

```sh
rm -f /tmp/arch.excalidraw
```

### 1. Author a Multi-Zone Architecture Diagram with `excal draw batch`

Run `excal draw batch` to create an entry gateway, two backend services connected by straight and elbow arrows, and an enclosing `Backend Tier` zone in a single atomic transaction:

```sh
cargo run --quiet -- draw batch /tmp/arch.excalidraw --in-place <<'EOF'
node --id gw --label "API Gateway" --x 100 --y 100
node --id auth --label "Auth Service" --right-of gw --from gw --edge-label "Verify"
node --id billing --label "Billing Service" --below auth --from gw --elbow
zone --id backend --title "Backend Tier" --children auth,billing
EOF
```

`excal draw batch` auto-initializes `/tmp/arch.excalidraw`, sizes each node to its label, spaces `auth` wide enough for the `"Verify"` edge label, wraps `auth` and `billing` in `backend`, and prints:

```text
excal: added 6 item(s), updated 0 item(s), removed 0 item(s)
excal: lint after change: errors: 0, warnings: 0, info: 0
```

### 2. Inspect the Reconstructed Scene Graph

Run `excal inspect` on `/tmp/arch.excalidraw` to see the container hierarchy, 4-way corridor neighbors, vertical column track, connectors, and ASCII minimap:

```sh
cargo run --quiet -- inspect /tmp/arch.excalidraw
```

Observable output on `stdout`:

```text
# excal inspect /tmp/arch.excalidraw
canvas x100..612 y25..360 | components: 3, containers: 1, connectors: 2 | errors: 0, warnings: 0, info: 0

## Containers
- backend zone "Backend Tier" x304..612 y25..360 parent=- padding T75 R40 B40 L40 children=[auth, billing]

## Components
- gw "API Gateway" rectangle x100..280 y100..180 c(190,140) parent=- L=- R=- T=- B=-
- auth "Auth Service" rectangle x362..554 y100..180 c(458,140) parent=backend L=- R=- T=- B=billing(60)
- billing "Billing Service" rectangle x344..572 y240..320 c(458,280) parent=backend L=- R=- T=auth(60) B=-

## Rows
(none)

## Columns
- backend x≈458: auth --60-- billing

## Connections
- gw-auth gw -> auth "Verify" straight (286,140) -> (356,140)
- gw-billing gw -> billing elbow (286,140) -> (312,140) -> (312,280) -> (338,280)

## Minimap
                         .......................................
                         .                                     .
                         .                                     .
                         .                                     .
AAAAAAAAAAAAAAAAAAAAAAA  .      BBBBBBBBBBBBBBBBBBBBBBBBB      .
AAAAAAAAAAAAAAAAAAAAAAA  .      BBBBBBBBBBBBBBBBBBBBBBBBB      .
AAAAAAAAAAAAAAAAAAAAAAA  .      BBBBBBBBBBBBBBBBBBBBBBBBB      .
AAAAAAAAAAAAAAAAAAAAAAA*********BBBBBBBBBBBBBBBBBBBBBBBBB      .
AAAAAAAAAAAAAAAAAAAAAAA  .*     BBBBBBBBBBBBBBBBBBBBBBBBB      .
AAAAAAAAAAAAAAAAAAAAAAA  .*     BBBBBBBBBBBBBBBBBBBBBBBBB      .
                         .*                                    .
                         .*                                    .
                         .*                                    .
                         .*   CCCCCCCCCCCCCCCCCCCCCCCCCCCCCC   .
                         .*   CCCCCCCCCCCCCCCCCCCCCCCCCCCCCC   .
                         .****CCCCCCCCCCCCCCCCCCCCCCCCCCCCCC   .
                         .    CCCCCCCCCCCCCCCCCCCCCCCCCCCCCC   .
                         .    CCCCCCCCCCCCCCCCCCCCCCCCCCCCCC   .
                         .    CCCCCCCCCCCCCCCCCCCCCCCCCCCCCC   .
                         .                                     .
                         .......................................
A=gw B=auth C=billing

## Issues
(none)
```

### 3. Add a Node That Causes an Overlap and Repair It with `excal layout`

Suppose an agent places a new `cache` node at explicit coordinates `(--x 400 --y 120)` inside the `backend` zone, colliding with `auth`:

```sh
cargo run --quiet -- draw node /tmp/arch.excalidraw --id cache --label "Redis Cache" --x 400 --y 120 --in-place
```

`excal draw` persists the node (exiting `0`) and immediately surfaces the overlap error and near-miss alignment warning on `stderr`:

```text
excal: added 1 item(s), updated 0 item(s), removed 0 item(s)
excal: lint after change: errors: 1, warnings: 1, info: 0
ERROR OVERLAP_COMPONENT auth "Auth Service" overlaps cache "Redis Cache" by 154x60px
      fix: move cache down by 100px to leave a 40px gap
      run: excal layout move /tmp/arch.excalidraw --id cache --dy 100 --ripple
WARN  NEAR_MISS_ALIGNMENT cache "Redis Cache" and billing "Billing Service" are misaligned by 8px (right)
      fix: align billing to cache (right)
      run: excal layout align /tmp/arch.excalidraw --ids cache,billing --axis right
```

Reposition `cache` to the right of `auth` with `excal layout place` so it sits cleanly inside `backend` and automatically expands the `backend` zone rectangle:

```sh
cargo run --quiet -- layout place /tmp/arch.excalidraw --id cache --right-of auth --in-place
```

Observable output on `stderr`:

```text
excal: moved 1 item(s), resized 1 item(s), updated 0 connector(s)
excal: lint after change: errors: 0, warnings: 0, info: 0
```

---

## Verify the Result

1. Run `excal lint --strict` against `/tmp/arch.excalidraw`:

    ```sh
    cargo run --quiet -- lint /tmp/arch.excalidraw --strict
    ```

2. Confirm that the output reports zero issues and exits with code `0`:

    ```text
    errors: 0, warnings: 0, info: 0
    ```

3. Clean up the temporary file:

    ```sh
    rm -f /tmp/arch.excalidraw
    ```

---

## Troubleshooting

- **`excal: error: node placement requires --x and --y or one of --right-of, --left-of, --below, --above` (exit code 2)**:
  Omitting coordinates works only when the canvas is empty or when `--in <CONTAINER>` is passed. Supply `--right-of <ID>`, `--below <ID>`, `--in <CONTAINER>`, or explicit `--x <N> --y <N>`.
- **`excal: error: id "<ID>" already exists` (exit code 2)**:
  Each node, container, or connector ID must be unique across the document. Use `excal draw label --id <ID> --text "..."` to rename an existing object, or `excal draw remove --id <ID>` to delete it before recreating it.
- **`excal: error: line N: ...` during `excal draw batch` (exit code 2)**:
  Batch scripts reject `--in-place`, `-o`, `init`, and nested `batch` flags inside script lines; pass `--in-place` or `-o <OUT>` on the outer `excal draw batch` invocation instead.

---

## Further Exploration

- [`docs/cli-reference.md`](cli-reference.md): complete lookup table of `excal` subcommands, flags, aliases, and exit codes.
- [`docs/architecture.md`](architecture.md): how `Scene::build` detects zones and how `retain_zone_membership` automatically expanded `backend` in Step 3.
- [`src/author/README.md`](../src/author/README.md): details on automatic shape sizing, edge-label gap floors, and parallel arrow spreading.
- [`src/mutator/README.md`](../src/mutator/README.md): details on `move --ripple`, `align`, `distribute`, `place`, `fit-container`, and `fit-text`.

---

## Dev Note

None.

# FloksRS

Native Rust counterpart to the canonical ULTRST Floks UI framework.

## Implemented (bounded parity increment)

- Render-independent authoring session nodes, parent hierarchy, subtree traversal, bindings and cascading deletion (including stale slot references).
- Typed editable graph nodes, socket compatibility, edges, node moves and deletion.
- Viewport zoom-at-point with cursor preservation and safe numeric validation.
- Native regression fixtures for session/graph behavior and CI (fmt, test, clippy).

Upstream pinned: `dinilmusick/Floks@5e05c8ce8cadf40b37deeaea42d8b60afe6f0576`, specifically
`packages/canvas-ui/src/authoring/{types,authoringModel,authoringOperations}.ts`
and `packages/contracts/src/state-graph.ts`.

This crate contains native contracts, **not** a React renderer or an executable scheduler.
It does not claim full Floks functional parity: missing React component rendering, native
widgets, full ghost graph/prediction behavior, automated browser checks and consuming-app
integration. The public CI verifies only implemented Rust contracts.

# FloksRS — verified native compatibility matrix

Pinned sources (22 November 2026):
- Canonical TypeScript/React Floks: `dinilmusick/Floks@7ab84526d7d9820a14de47b0ab33b568b8367460`
- Semantic Rust dependency: `dinilmusick/FrnlRS@4325cf84fdad2a51ea41c10bf049342fee2a7327`
- Utility reference only: `dinilmusick/GutRS@3e4ad1823166b37ddc437aab04277b42c124ce36`
- Wave 3 baseline: `0c5134203c03858f396ed09db3d3f32a6334b746`
- Crate revision: `0.3.0`

| Canonical capability | Rust implementation | Scope |
| --- | --- | --- |
| Authoring sessions, tree, binding, editable nodes | AuthoringSession and `codec::import/export_authoring` | Canonical camelCase fields; structural, cycle and stale binding validation |
| Authoring operations, assets, primitive props | AuthoringOperation/AssetRef/Value | Full operation metadata; integers and floating values remain distinct |
| Versioned authoring snapshots | `codec::import/export_authoring_document` | Strict schemaVersion 1, while the flat canonical payload remains unchanged |
| Component registry and control actions | `controls::Registry` | Button, tile, text, toggle, selector, menu, tabs, dialog; lifecycle tree, state, events |
| Versioned registry snapshots | `codec::import/export_registry` | Deterministic sorted snapshot and stale/cyclic widget validation; queued ephemeral events not persisted |
| Typed graph nodes, sockets, viewport | Graph, Viewport | Data model, socket compatibility, positions, parenting and safe deletion |
| Graph editor history, cycle rules and selection | `graph_edit::Editor` | Real undo/redo, selection, atomic movements and acyclic edit validation |
| Versioned graph JSON | `codec::import/export_graph` | Native semantic graph projection and valid socket/edge enforcement |
| Lossless canonical graph metadata | `codec::GraphDocument` | Preserves original labels, visual extras, data, viewport fields, and unknown presentation metadata. Converting to the smaller native Graph explicitly projects only semantic fields |
| FrnlRS action and state integration | `semantic_bridge::Bridge` | Real Runtime handlers, state, event history, lifecycle transitions, suspend/resume and subtree disposal |
| Theme and layout | `theme::{Theme, Layout}` | Light/dark/system appearance and deterministic row/column descriptors |
| Native pixel renderer | `framebuffer::Framebuffer` feature | Actual RGB888 pixel writes into a host-owned framebuffer, visible glyphs and theme-aware controls |
| Piano roll and musical sections | `music::{PianoRoll, Geometry, Timeline}` | All 128 MIDI pitches, notes, ticks, zoom, playhead, loop, quantization and sections |
| MIDI and OSC wire protocols | `host_io` feature | Validated MIDI bytes, OSC 1.0 big-endian packets and real host-supplied port callbacks |
| Embedded portable targets | GitHub CI | thumbv7em-none-eabihf and wasm32-unknown-unknown no-default-features library builds |
| Independent external Rust application | `examples/external-consumer` | Real FrnlRS callback, MIDI/OSC host byte capture and framebuffer output |

## Platform boundaries (not fake implementations)

FloksRS does not contain a GUI operating-system event loop or hardware MIDI device driver. Hosts provide their own display compositor, platform window, native I/O transport, clock and resource lifetime. The opt-in framebuffer does draw pixels into a real caller-provided buffer but does not claim to open a graphical window. Embedded target builds prove toolchain and library portability, not execution on physical hardware or firmware linked to a board. Browser-only React, Monaco and SVG presentation internals are platform-specific exceptions rather than concepts to clone.

Native `Graph` is a deliberate semantic projection; use `GraphDocument` when round-tripping rich TypeScript visual metadata without loss. Native FloksRS 0.3.0 changes the previous operation-string representation to full operation records, matching the canonical TypeScript shape.

Release certification: GitHub Actions must pass rustfmt, all-target Rust tests, Clippy with denied warnings, no-default-features, feature tests, two cross-target library builds and standalone external consumer. PR exact-head and postmerge main checks are separate required gates. Ledger verification is independent of the implementing worker.

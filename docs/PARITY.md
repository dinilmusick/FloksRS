# FloksRS Wave 3 native compatibility matrix

Pinned source: Floks 7ab84526d7d9820a14de47b0ab33b568b8367460.
FrnlRS: 4325cf84fdad2a51ea41c10bf049342fee2a7327.
GutRS: 3e4ad1823166b37ddc437aab04277b42c124ce36 (reference; no direct dependency).
Baseline PR #1 merged as 638cbb27a463a92270c5869f633403097206add6.

| Floks capability | Native Rust status | Limitation |
| --- | --- | --- |
| Authoring components, tree, bindings | AuthoringSession | No upstream-compatible JSON serializer |
| Button, text, toggle, tile, menu, selector, tabs, dialog | controls Registry | Semantic interactions, no rendering |
| Actual FrnlRS event/availability | Registry mount_frnl | Host callback; deeper state-machine mapping pending |
| Theme, light/dark/system tokens | theme Theme | No browser CSS |
| Geometry and layout descriptors | theme Layout | Basic linear layout, host renderer required |
| Typed sockets, node graph, viewport | Graph and Viewport | Baseline implemented |
| History, selection, acyclic links | graph_edit Editor | Model only |
| MIDI piano roll, 128 pitches, note movement and zoom | music PianoRoll and Geometry | No actual audio/hardware device |
| Playhead, loop and quantization | music PianoRoll | Tick-based contract |
| Section timeline | music Timeline | Tick-based model |
| Native GUI bridge | NativeRenderer trait | No concrete renderer shipped |
| React and Monaco editor, ghost visual overlays | Platform exception | Browser-specific visuals are not ported |
| Embedded device validation | No-default Rust check only | Actual target/linker smoke outstanding |
| External consuming application | Separate Cargo package | Verifies real FrnlRS Surface callback in CI |

This is not a graphical GUI renderer. Exact-head CI and post-merge checks determine release certification.

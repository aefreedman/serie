# Serie Plastic (read-only MVP)

A local Plastic SCM / Unity Version Control history browser, adapted from
[Serie](https://github.com/lusingander/serie) by Kyosuke Fujimoto. Upstream MIT
license and notices are retained in [LICENSE](LICENSE).

## Run

Requires Rust 1.88+ to build and a trusted installed `cm` on PATH, authenticated
for the workspace repository. Tested with Windows and cm 11.0.16.10371.
`cm` is launched directly through PATH on Windows, macOS, and Linux, without a
shell or a hardcoded install location; workspace paths may contain spaces.

```powershell
cargo build --locked
.\target\debug\serie.exe --workspace "C:/path/to/workspace" --protocol text
```

On macOS/Linux:

```sh
cargo build --locked
./target/debug/serie --workspace "/path/to/workspace" --protocol text
```

The workspace defaults to the current directory. It is used only as the cwd for
read-only `cm status`, repository-scoped `cm find`, and lazy `cm log` queries.
No switch/update/merge/checkin or filesystem writes are performed there.
The production executable does not include the legacy Git query implementation;
upstream Git graph regression tests operate only in temporary test repositories.
External user commands (including configured commands) and custom clipboard
subprocesses are disabled. Clipboard copying uses the system clipboard library.
There is no default Git diff command. Historical diffs are deferred.

## Headless validation

No terminal, image protocol, config file, clipboard, or Git installation is needed:

```powershell
.\target\debug\serie.exe --workspace "C:/path/to/workspace" --dump --detail 16
.\target\debug\serie.exe --workspace "C:/path/to/workspace" --dump --max-count 2
```

`--dump` prints deterministic qualified changeset selectors, full numeric IDs,
metadata, primary edges, separately typed integration links (including base),
branch/label producer annotations, the original workspace identity, graph marker,
truncation flags, missing endpoints, and warnings. `--detail NUMBER` adds raw
changed-path status, source/destination paths, revision and parent revision IDs;
it must be inside the loaded window. Errors exit nonzero rather than returning
partial detail success. Dump comments/paths are escaped, not shell commands.

Default limits: 500 changesets, 2000 integrations, 2000 references per kind;
`--max-count` accepts 1..10000. Each cm command has a 30-second deadline, 8 MiB
stdout and 64 KiB stderr cap. Failed, oversized, invalid UTF-8/XML output fails
explicitly. Refresh reruns the same bounded load and rechecks workspace selection.

## Portability checks

CI runs `cargo fmt --all -- --check`, `cargo test --locked`, and
`cargo build --locked` on native Windows, macOS, and Linux runners with stable
Rust and the Cargo manifest's minimum Rust version (1.88). CI needs Git for
temporary Git regression repositories, but **no cm installation, credentials,
real workspace, terminal, or clipboard session**. Backend process tests use an
isolated fake executable, including PATH lookup and workspace paths with spaces.
Graph image tests use an embedded font, not system fonts.

On Debian/Ubuntu, install `pkg-config` and `libwayland-dev` before building
(the clipboard dependency includes Wayland support). Windows/macOS use their
native clipboard APIs; Linux clipboard operations require a working X11 or
Wayland desktop session and can fail on headless hosts. Clipboard initialization
is lazy, so headless dumps and tests do not require desktop access.

```sh
cargo fmt --all -- --check
cargo test --locked
cargo build --locked
```

The native CI matrix is prospective coverage, **not evidence of completed
macOS/Linux runs**. Local validation was Windows only. Inherited cross-target
builds are retained as manual, nonblocking workflow jobs and remain unverified
for this port; inherited clippy warnings are reported by a nonblocking job.
No release/publication workflow is changed by this pass.

## TUI

- `j`/`k` or arrows: navigate; `g`/`G`: first/last; Page Up/Down: page.
- Enter: metadata and lazy changed paths; Backspace/Esc: close.
- `/`: search; `n`/`N`: next/previous match; Ctrl-T: search field;
  Ctrl-G: case handling; Ctrl-X: fuzzy matching.
- Tab: branches/labels; `R`: refresh; `?`: help; `q` or Ctrl-C: quit.
- `c` and `C`: copy the full qualified changeset selector (no hash slicing).
  In the ref list, copy the qualified branch/label selector.
- `--initial-selection head`: select the loaded workspace changeset if visible;
  the graph marks it `LOADED`, independent of branch annotations.

`--protocol text` draws Unicode nodes/edges directly in Ratatui, with no image
escapes/uploads. It uses the configured graph palette for nodes and edges.
Every changeset uses a distinct `●` node marker. Non-node edge segments use light
strokes and composed junctions, with continuity on intervening rows and horizontal
continuations in double-width mode. A node occupies one text cell, so connection
arms cannot also be drawn in that cell: vertically adjacent nodes can appear
separated depending on the terminal/font. This text-cell limitation remains
unresolved; dots are not replaced by strokes to hide it.
Auto chooses text on ordinary terminals (including Windows),
Kitty on detected Kitty-compatible terminals, or iTerm images on detected iTerm.
Explicit `--protocol iterm`, `kitty`, and `kitty-unicode` remain available.
Terminal graphics and an actual interactive Windows terminal session have **not**
been verified; text row construction, navigation/search regressions, and headless
loading have automated coverage. Font/terminal behavior can still vary.

Existing Serie UI/keybind/config options remain for compatibility. Git-specific
mailmap/order settings are inert; there is no Git backend CLI selection. Some
internal config/search terminology still uses "commit", "hash", or "tag";
these mean changeset number and label on the Plastic path.

## Honest graph boundaries

The drawing represents **explicit primary parents plus ordinary merge integration
links**. Primary parents are kept first; ordinary merges add deduplicated layout
edges only when both qualified endpoints are loaded. Merge edges use the existing
merge layout/style (not a separate color legend). The original primary parent is
identified explicitly in metadata, and typed integration records remain separate
in the headless dump and endpoint details with their optional base. Cherry-pick,
subtractive, interval, and unknown integration types are omitted from the drawing
with a warning, not misrepresented as ordinary ancestry. Branch hierarchy and
consecutive changeset numbers do not imply edges. Ordinary merge links with
out-of-window endpoints are also disclosed and not drawn.

Branch/label annotations use the producer's `CHANGESET` field, not inferred
branch tips. References without supported qualified targets are disclosed and
retained in the dump, not invented as graph nodes. Out-of-window targets/parents
remain boundaries; no placeholder changesets are fabricated. The installed cm
returns different server identities for status and scoped find; the backend
retains original status identity and reports its scoped alias mapping warning.
All backend warnings are shown at load and again in detail views.

Single qualified repository only. Sequential server queries are not a
transactional snapshot; concurrent checkins/reference edits can race. Live
fixtures cover labels using cm's MARKER records, including a label on sandbox
changeset 16. Nonordinary integration types have synthetic test coverage only. Oversized details fail, not paginate.
Direct child execution is bounded, but there is no hostile process-tree
containment: trusted cm is assumed. Unsupported parent ordering/cycles fail
explicitly before graph rendering.

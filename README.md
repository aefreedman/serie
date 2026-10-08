# Serie Plastic

A terminal history browser for Plastic SCM / Unity Version Control, adapted from [Serie](https://github.com/lusingander/serie) by Kyosuke Fujimoto. The upstream MIT license and notices are preserved in [LICENSE](LICENSE).

Browse changeset graphs, metadata, changed files, branches, and labels. Search history, copy qualified selectors, or export a headless dump. The built-in backend is read-only: it does not switch workspaces, update files, merge, or check in changes.

**Built-in diffs are not included.** Use Plastic's GUI for file and changeset comparisons.

## Quick start: Windows

**0.9.3-plastic.1** is the preliminary Plastic port, based on upstream Serie 0.9.3. Windows x64 is the initial binary target; see [release notes](RELEASE_NOTES.md) for packaging details and limitations.

You need Plastic's `cm` CLI installed on `PATH` and authenticated for your repository. It is not bundled. Extract the release zip into its own directory, then run:

```powershell
.\serie.exe --version
.\serie.exe --workspace "C:/path/to/workspace" --protocol text
```

No installer or launcher changes are needed. If you omit `--workspace`, Serie uses the current directory.

The interactive browser has been used successfully on Windows with `cm` 11.0.16.10371. macOS and Linux runtime behavior is not yet verified; source builds on those platforms are experimental.

## Using the browser

| Key | Action |
| --- | --- |
| `j` / `k`, arrow keys | Move through history |
| `g` / `G` | Go to the first / last row |
| Page Up / Page Down | Move one page |
| Enter | Open metadata and load changed files |
| Backspace / Esc | Close details |
| `/` | Search |
| `n` / `N` | Next / previous match |
| Ctrl-T / Ctrl-G / Ctrl-X | Change search field / case handling / fuzzy matching |
| Tab | Browse branches and labels |
| `c` / `C` | Copy the full qualified selector |
| `R` | Refresh |
| `?` | Show help |
| `q` / Ctrl-C | Quit |

In the reference list, copying uses the qualified branch or label selector. `--initial-selection head` selects the workspace's loaded changeset when it is in the current window. That changeset is marked `LOADED`, independently of branch annotations.

### Terminal rendering

`--protocol text` draws Unicode nodes and edges directly in the terminal, without image uploads. It uses the configured graph colors and a `●` for each changeset. Because a node occupies a whole text cell, adjacent nodes can look disconnected in some fonts or terminals.

Automatic protocol selection uses text on ordinary terminals, including Windows; Kitty images on detected Kitty-compatible terminals; and iTerm images on detected iTerm terminals. You can also select `iterm`, `kitty`, or `kitty-unicode` explicitly. Not all graphics protocols have been verified interactively.

### Configuration and external commands

Existing Serie keybindings, UI options, and configuration remain available. Git mailmap settings have no effect, and there is no Git backend option. Some inherited configuration and search names still say “commit”, “hash”, or “tag”; on this port they refer to changesets, changeset numbers, or labels.

Clipboard copying uses the system clipboard by default (`auto`). A configured [external clipboard command](docs/src/configurations/config-file-format.md#coreexternalclipboard) instead receives the exact qualified selector as UTF-8 on stdin, without an added newline. It runs without a shell and reports failures, but has no timeout.

Optional [user commands](docs/src/features/user-command.md) run in the selected workspace. None are configured by default, including diff commands. **Only configure commands you trust:** user commands and external clipboard programs can modify files or have other effects, even though the history backend is read-only.

## Changeset ordering

Choose `--order chrono|topo` (or `-o`) to override `[core.option] order` in your configuration. The default is `chrono`. Ordering applies to the initial load, refresh, and headless dumps.

- **`chrono`** picks the newest eligible changeset. Children appear before their primary and ordinary merge parents, even when timestamps are out of order.
- **`topo`** starts at the newest eligible tip and follows newly available parents depth-first, preferring primary parents over ordinary merge parents. Shared parents wait for all loaded children. This keeps branch runs together where ancestry allows; it does not group by branch name.

Timestamp comparisons account for timezone offsets. Ties use descending numeric changeset IDs, then qualified selectors. Merge-parent traversal is also deterministic. Neither ordering mode changes the query window or fills in missing history.

## What the graph shows

Graph edges represent **explicit primary parents and ordinary merges**, and only when both qualified endpoints are loaded. Primary parents remain identifiable in metadata; typed integration records and their optional bases are also available in details and dumps.

Cherry-pick, subtractive, interval, and unknown integrations are reported with warnings rather than drawn as ordinary ancestry. Branch hierarchy and consecutive changeset numbers do not imply edges. Cycles in loaded primary/ordinary merge ancestry cause an error before rendering.

Branch and label annotations use the server's `CHANGESET` field, not inferred branch tips. Unsupported reference targets and out-of-window parents or targets are reported, not replaced with invented graph nodes. Warnings appear when history loads and again in detail views.

### Limits

- One qualified repository at a time.
- By default, up to 500 changesets, 2,000 integrations, and 2,000 references per kind. `--max-count` accepts 1–10,000 changesets.
- Each `cm` command has a 30-second deadline, an 8 MiB stdout limit, and a 64 KiB stderr limit. Failed commands, oversized output, and invalid UTF-8 or XML produce errors. Oversized details are not paginated.
- Refresh repeats the bounded load and rechecks the selected workspace. Sequential server queries are not a transactional snapshot: concurrent check-ins or reference edits can affect the results.
- When workspace status and scoped queries return different server identities, the backend preserves the original workspace identity and warns about the alias mapping.

Serie assumes a trusted `cm` executable. It launches `cm` directly through `PATH`, without a shell or a hardcoded installation path, and supports workspace paths containing spaces. Execution limits apply to the direct child process; they do not provide containment for a hostile process tree.

## Headless inspection

Use `--dump` to inspect history without a terminal, graphics protocol, clipboard, or Git installation:

```powershell
.\serie.exe --workspace "C:/path/to/workspace" --dump --max-count 2
.\serie.exe --workspace "C:/path/to/workspace" --dump --detail 16
```

A configuration file is optional. If present, it is validated, and its ordering setting applies unless overridden by `--order`.

The dump includes qualified selectors, full numeric IDs, metadata, primary edges, typed integration links and bases, branch/label annotations, workspace identity, graph markers, truncation flags, missing endpoints, and warnings. Rows follow the selected ordering; metadata retains the server's values. Comments and paths are escaped, not emitted as shell commands.

`--detail NUMBER` adds changed-path status, source and destination paths, and revision and parent revision IDs. The changeset must be in the loaded window. Detail errors exit nonzero rather than returning partial success.

## Build from source

Requires **Rust 1.88+**. To browse a real repository, you also need an installed, trusted, authenticated `cm` on `PATH`.

Windows:

```powershell
cargo build --locked
.\target\debug\serie.exe --workspace "C:/path/to/workspace" --protocol text
```

macOS / Linux (experimental):

```sh
cargo build --locked
./target/debug/serie --workspace "/path/to/workspace" --protocol text
```

On Debian/Ubuntu, install `pkg-config` and `libwayland-dev` before building. Linux clipboard operations need a working X11 or Wayland desktop session; Windows and macOS use native clipboard APIs. Clipboard initialization is lazy, so headless dumps and tests do not need desktop access.

### Development checks

```sh
cargo fmt --all -- --check
cargo test --locked
cargo build --locked
```

Tests need Git for temporary upstream regression repositories, but do not need `cm`, credentials, a real workspace, a terminal, or a clipboard session. Backend process tests use an isolated fake executable, including tests for `PATH` lookup and workspace paths with spaces. Graph image tests use an embedded font. The production executable does not include the legacy Git query backend.

The CI configuration runs these checks on native Windows, macOS, and Linux runners with stable Rust and Rust 1.88. This describes the configured matrix, not completed runs: local validation for this release was Windows-only. Nonordinary integration types have synthetic test coverage only. Inherited cross-target builds are manual, nonblocking jobs; clippy is also nonblocking.

The release workflow packages Windows x64 for the explicit Plastic preliminary-release tag. It creates a draft prerelease, does not mark it as latest, and does not publish to Cargo. Publishing the draft is a separate step.

# User Command (Plastic)

Only explicitly configured operator commands execute. There is no built-in Git,
diff or mutation command. The history backend remains read-only; configured
commands are **trusted operator intent**, can have effects, and are not restricted
to read-only operations. Do not configure untrusted executables or arguments.
Custom clipboard subprocesses remain disabled in the Plastic entry point.

Commands are argv arrays, launched directly without a shell, with the selected
`--workspace` as cwd (including silent and suspended commands). Shell syntax is
literal unless you explicitly configure a shell. Empty executables are rejected.

```toml
[keybind]
user_command_1 = ["d"]

[core.user_command]
commands_1 = { name = "Inspect changeset", commands = ["my-read-only-helper", "{{changeset}}", "{{primary_parent}}", "{{labels}}"] }
```

The example helper is operator-supplied, not bundled. Existing Serie modes remain:

- `inline` (default): captures stdout in the command view; ANSI color and tabs
  are supported. Selecting another changeset reruns it; the same binding closes it.
- `silent`: captures/discards output without opening a view; waits for completion
  (background means no output view, not an asynchronous/detached process).
- `suspend`: leaves the TUI/raw mode for an interactive command, then restores it.

Spawn/nonzero failures are reported. Silent refresh occurs on success; suspend
refresh occurs after the attempted execution, even on failure, matching Serie.
`refresh = true` reloads the bounded repository for silent/suspend commands; it
is invalid for inline commands. Manual refresh reloads and reruns an inline view.
No deadlines/output caps are imposed on operator commands; they can block the UI.

## Placeholders

| Placeholder | Plastic value / legacy alias |
| --- | --- |
| `{{changeset}}` | Selected qualified `cs:17@rep:demo@repserver:server:8087`; alias `{{target_hash}}` |
| `{{primary_parent}}` | Explicit producer primary parent selector, not the first merge layout edge; alias `{{first_parent_hash}}` |
| `{{parents}}` | Primary parent followed by deduplicated ordinary merge sources from loaded evidence; alias `{{parent_hashes}}` |
| `{{branches}}` | Qualified `br:/main@rep:demo@repserver:server:8087` annotations pointing at the selected changeset |
| `{{labels}}` | Qualified `lb:release one@rep:demo@repserver:server:8087` annotations; alias `{{tags}}` |
| `{{refs}}` | Branch and label annotations above |
| `{{remote_branches}}`, `{{stash}}` | Empty; no Plastic counterpart |
| `{{area_width}}`, `{{area_height}}` | Output area dimensions in cells |

Primary-parent absence produces an empty string (including a root with ordinary
merge evidence). A known parent outside the history window retains its qualified
selector. Parent collections include evidenced ordinary merge sources even outside
the window; typed nonordinary integrations are not parents. Bounded reads can omit
merge/ref evidence; collections do not imply complete repository history.

Standalone collection placeholders expand to separate argv items, preserving
spaces and exact qualifiers. Empty collections remove that argument. Embedded
collections join with spaces in one argument. Missing scalar values remain an
empty argument. Unknown placeholders remain literal. No shell quoting or escaping
is added, and substituted values are not reinterpreted as placeholders.

Native macOS/Linux execution and interactive terminal lifecycle remain unverified
by local Windows unit tests. Validation uses only benign temporary helpers, not
mutation commands on sandbox or real workspaces.

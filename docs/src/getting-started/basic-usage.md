# Basic Usage

Run the locally built Plastic port in an authenticated Unity Version Control
workspace, or specify its path:

```sh
serie --workspace "/path/to/workspace" --protocol text
serie --workspace "/path/to/workspace" --dump --max-count 2
```

The built-in backend is read-only. The default window is 500 changesets, not all
history. Enter loads metadata and changed paths; Tab opens branches and labels;
`R` refreshes the same bounded window. `c`/`C` copy the full qualified changeset
selector in list/detail, or qualified branch/label selector in refs. There is no
default diff or user command; `d` only works when explicitly configured. Trusted
user/clipboard commands can have effects.

See [Command Line Options](./command-line-options.md) and
[User Command](../features/user-command.md).

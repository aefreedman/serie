# FAQ

## Why doesn't the graph display?

Use `--protocol text` to draw Unicode changeset graphs without terminal image
support. Auto falls back to text on ordinary terminals. Image protocols still
require a compatible terminal; upstream compatibility reports are not verified
port evidence. Adjacent text nodes may appear separated because a node cell
cannot also draw connection arms. This rendering limitation remains deferred.
See [Compatibility](../getting-started/compatibility.md).

## What are the advantages over other git TUI clients?

The Plastic port is a read-only changeset browser, not a Git client. The following
comparison describes upstream Serie rather than active Git capabilities here:

- High-quality graph visualization using terminal graphics protocols
- Simple and clean interface

On the other hand, Serie may not be for you if:

- You're satisfied with `git log --graph` or the graph display in existing TUI clients
- You need to perform complex git operations within a TUI client

## How do I pronounce "Serie"?

It is pronounced as the German word Serie (**/ˈzeːriə/**), roughly like **"ZAY-ree-eh"**, not like the English "series".

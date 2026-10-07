# Command Line Options

## -n, --max-count \<NUMBER\>

Maximum changesets to load: `1..10000`, default **500**. This intentional bounded
newest-ID window is not full history. Integrations and references are separately
bounded (2000 integrations; 2000 references per kind). Missing endpoints and
truncation are disclosed; refresh uses the same limits.

## -p, --protocol \<TYPE\>

A rendering protocol for changeset graphs, including text without image escapes.

_Possible values:_ `auto`, `text`, `iterm`, `kitty`, `kitty-unicode`

`auto` selects detected Kitty/iTerm images, otherwise Unicode `text` (including
ordinary Windows terminals). Explicit `text` avoids image uploads. See
[Compatibility](./compatibility.md) for validation limitations.

## -o, --order \<TYPE\>

Changeset ordering within the bounded window. CLI overrides `[core.option] order`;
default `chrono`. Both modes emit loaded children before primary and ordinary
merge parents; other integration types remain evidence, not ancestry.

_Possible values:_ `chrono`, `topo`

`chrono` chooses the newest timestamp among eligible changesets, not a naive
date sort. Timestamp skew cannot place a loaded parent before its child.

<img src="https://raw.githubusercontent.com/lusingander/serie/master/img/order-chrono.png" width=300>

`topo` follows newly unblocked ancestry depth-first (primary before ordinary merge
parents), then returns to the newest eligible tip. Shared parents wait for all
loaded children. It is not branch-name grouping. Date priorities compare instants
including timezone offsets; ties use descending numeric IDs then selectors.
Neither mode changes the query window or fabricates ancestry. The images on this
page illustrate upstream Git behavior, not live Plastic evidence.

<img src="https://raw.githubusercontent.com/lusingander/serie/master/img/order-topo.png" width=300>

## -g, --graph-width \<TYPE\>

The character width that a graph image unit cell occupies.

_Possible values:_ `auto`, `double`, `single`

If not specified or `auto` is specified, `double` will be used automatically if there is enough width to display it, `single` otherwise.

<img src="https://raw.githubusercontent.com/lusingander/serie/master/img/graph-width-double.png" width=300>

<img src="https://raw.githubusercontent.com/lusingander/serie/master/img/graph-width-single.png" width=300>


## -s, --graph-style \<TYPE\>

The commit graph image edge style.

_Possible values:_ `rounded`, `angular`

`rounded` will use rounded edges for the graph lines.

<img src="https://raw.githubusercontent.com/lusingander/serie/master/img/graph-width-double.png" width=300>

`angular` will use angular edges for the graph lines.

<img src="https://raw.githubusercontent.com/lusingander/serie/master/img/style-angular.png" width=300>

## -i, --initial-selection \<TYPE\>

The initial selection of commit when starting the application.

_Possible values:_ `latest`, `head`

`latest` selects the first changeset in the chosen order.

`head` selects the loaded workspace changeset if visible (`LOADED` marker); it
does not switch or update the workspace.

## -b, --primary-branch \<BRANCH\>

The primary branch to keep on the leftmost column.

When specified (for example `/main`), Serie follows the loaded primary-parent
spine from that branch's producer `CHANGESET` annotation on the leftmost column.
This does not infer a branch tip from names or consecutive changeset numbers.
An unresolved or out-of-window annotation produces a warning; no missing nodes
are fabricated. It changes layout, not history ordering.

## --workspace <PATH>

Workspace cwd for read-only `cm status`, scoped `cm find`, and lazy `cm log`.
Defaults to the current directory; quoted paths may contain spaces.

## --dump and --detail <NUMBER>

`--dump` prints qualified selectors, metadata, graph rows, primary edges, typed
integrations, branch/label annotations, missing endpoints, and warnings without
a terminal or clipboard. An optional config is validated; ordering still applies.
`--detail NUMBER` requires `--dump` and a changeset inside the loaded window; it
adds raw changed-path evidence. Errors exit nonzero. Dump text is not shell code.

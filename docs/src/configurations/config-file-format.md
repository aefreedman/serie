# Config File Format

## Example

This is an opt-in example, not the default configuration. The `cm version`
command is read-only; remove `commands_1` to leave user commands unconfigured.

```toml
[core.option]
protocol = "text"
order = "chrono"
graph_width = "auto"
graph_style = "rounded"
initial_selection = "latest"

[core.git]
mailmap = false

[core.search]
target = "all"
ignore_case = false
fuzzy = false

[core.user_command]
commands_1 = { name = "cm version", commands = ["cm", "version"] }
tab_width = 4

[core.external]
clipboard = "auto"

[ui.common]
cursor_type = "native"

[ui.list]
scrolloff = 0
columns = ["graph", "marker", "subject", "name", "hash", "date"]
subject_min_width = 20
date_format = "%Y-%m-%d"
date_width = 10
date_local = true
name_width = 20

[ui.detail]
height = 20
date_format = "%Y-%m-%d %H:%M:%S %z"
date_local = true

[ui.user_command]
height = 20

[ui.refs]
width = 26

[graph]
row_image_width = "compact"

[graph.color]
branches = [
  "#E06C76",
  "#98C379",
  "#E5C07B",
  "#61AFEF",
  "#C678DD",
  "#56B6C2",
]
edge = "#00000000"
background = "#00000000"

[color]
fg = "reset"
bg = "reset"
list_selected_fg = "white"
list_selected_bg = "dark-gray"
list_ref_paren_fg = "yellow"
list_ref_branch_fg = "green"
list_ref_remote_branch_fg = "red"
list_ref_tag_fg = "yellow"
list_ref_stash_fg = "magenta"
list_head_fg = "cyan"
list_subject_fg = "reset"
list_name_fg = "cyan"
list_hash_fg = "yellow"
list_date_fg = "magenta"
list_match_fg = "black"
list_match_bg = "yellow"
detail_label_fg = "reset"
detail_name_fg = "reset"
detail_date_fg = "reset"
detail_email_fg = "blue"
detail_hash_fg = "reset"
detail_ref_branch_fg = "green"
detail_ref_remote_branch_fg = "red"
detail_ref_tag_fg = "yellow"
detail_file_change_add_fg = "green"
detail_file_change_modify_fg = "yellow"
detail_file_change_delete_fg = "red"
detail_file_change_move_fg = "magenta"
ref_selected_fg = "white"
ref_selected_bg = "dark-gray"
help_block_title_fg = "green"
help_key_fg = "yellow"
virtual_cursor_fg = "reset"
status_input_fg = "reset"
status_input_transient_fg = "dark-gray"
status_info_fg = "cyan"
status_success_fg = "green"
status_warn_fg = "yellow"
status_error_fg = "red"
divider_fg = "dark-gray"

[keybind]
# See the separate Custom Keybindings section for details.
# ...
```

## Configuration Options

### `core.option.protocol`

The rendering protocol for changeset graphs. `text` draws Unicode cells without
image escapes; Auto selects detected Kitty/iTerm images, otherwise text.
Interactive terminal/font behavior remains unverified; adjacent nodes may appear
separated because a text node cell cannot also contain connection arms.

- type: `string` (enum)
- default: `auto`
- possible values:
  - `auto`
  - `text`
  - `iterm`
  - `kitty`
  - `kitty-unicode`

The value specified in the command line argument takes precedence.

### `core.option.order`

Changeset ordering within the intentional bounded window (default 500, CLI cap
10000). Both modes keep loaded children before primary and ordinary merge parents.
`chrono` chooses newest eligible timestamps; `topo` follows newly unblocked
ancestry depth-first before other tips, not branch-name grouping. See
[CLI ordering](../getting-started/command-line-options.md) for details.

- type: `string` (enum)
- default: `chrono`
- possible values:
  - `chrono`
  - `topo`

The value specified in the command line argument takes precedence.

### `core.option.graph_width`

The character width that a graph image unit cell occupies.

- type: `string` (enum)
- default: `auto`
- possible values:
  - `auto`
  - `double`
  - `single`

The value specified in the command line argument takes precedence.

### `core.option.graph_style`

The commit graph image edge style.

- type: `string` (enum)
- default: `rounded`
- possible values:
  - `rounded`
  - `angular`

The value specified in the command line argument takes precedence.

### `core.option.initial_selection`

The initial selection of changeset: `latest` is the first row in the selected
order; `head` is the loaded workspace changeset if visible (the `LOADED` marker).

- type: `string` (enum)
- default: `latest`
- possible values:
  - `latest`
  - `head`

The value specified in the command line argument takes precedence.

### `core.git.mailmap`

Compatibility-only Git setting; inert in the Plastic production backend.

- type: `boolean`
- default: `false`

Plastic owner metadata is retained; no Git mailmap lookup is performed.

### `graph.row_image_width`

The width mode for each graph row image.

- type: `string` (enum)
- default: `compact`
- possible values:
  - `compact`: trim each row image to the minimum required graph width
    - Reduces the file size for more lightweight operation. If you are using `kitty-unicode` protocol, it helps you avoid hitting [the image transfer size limit](https://sw.kovidgoyal.net/kitty/graphics-protocol/#image-persistence-and-storage-quotas).
  - `fixed`: use the same full graph width for every row image
    - This can be used when you want to set a background color for graphs in environments that cannot correctly handle transparent images, or in environments where rendering does not work well when there are images of various widths.

### `core.search.target`

The field to search when the application starts. The target can be toggled while the commit list is displayed.

- type: `string` (enum)
- default: `all`
- possible values:
  - `all`: Search branch/label refs, changeset subjects, owners, and full numeric changeset IDs
  - `subject`: Search changeset subjects
  - `author`: Search owner names
  - `ref`: Search branch and label names (no Git remotes/stashes)
  - `hash`: Search full numeric changeset IDs (`hash` is a compatibility name)

### `core.search.ignore_case`

Whether to enable ignore case when the application starts. The option can be toggled while the commit list is displayed.

- type: `boolean`
- default: `false`

### `core.search.fuzzy`

Whether to enable fuzzy matching when the application starts. The option can be toggled while the commit list is displayed.

- type: `boolean`
- default: `false`

### `core.user_command.commands_{n}`

The command definition for executing external commands.

Multiple commands can be specified in the format `commands_{n}`.
For details about user command, see the separate [User command](../features/user-command.md) section.

- type: `object`
- fields:
  - `name`: `string` - The name of the user command.
  - `type`: `string` (enum) - The type of user command.
    - default: `inline`
    - possible values:
      - `inline`: Display the output of the command in the user command view.
      - `silent`: Wait for the command without opening a view (not detached).
      - `suspend`: Execute the command by suspending the application. This is useful for interactive commands.
  - `commands`: `array of strings` - The command and its arguments.
  - `refresh`: `boolean` - Whether to reload the repository and refresh the display after executing the command. Available for `silent` and `suspend` commands.
    - default: `false`
- examples:
    - `commands_1 = { name = "cm version", commands = ["cm", "version"] }`
    - `commands_2 = { name = "cm help", type = "silent", commands = ["cm", "help"] }`
    - `commands_3 = { name = "cm version", type = "suspend", commands = ["cm", "version"] }`

No command is configured by default. The examples are opt-in and read-only; other
trusted commands can have effects and have no deadline/output cap.
  
### `core.user_command.tab_width`

The number of spaces to replace tabs in the user command output.

- type: `u16`
- default: `4`

### `core.external.clipboard`

The clipboard mechanism for qualified changeset/branch/label selector copying.
Explicit custom configuration is honored. Commands run directly as executable/argv
without a shell, receive exact UTF-8 stdin without an added newline, and report
spawn/write/wait/nonzero failures. Blank executables are rejected. Trusted custom
clipboard commands have no timeout and may block or have effects.

- type: `object` (enum)
- default: `auto`
- possible values:
  - `auto`: Use the default clipboard library
  - `{ custom = { commands = ["..."] } }`: Use a custom command that receives text via stdin
    - `commands`: `array of strings` - The command and its arguments.
- examples:
    - `clipboard = "auto"`
    - `clipboard = { custom = { commands = ["wl-copy"] } }`
    - `clipboard = { custom = { commands = ["xclip", "-selection", "clipboard"] } }`

### `ui.common.cursor_type`

The type of a cursor to display in the input.

- type: `object` (enum)
- default: `native`
- possible values:
  - `native`: Use the terminal native cursor.
  - `{ virtual = "|" }`: Use a virtual cursor with the specified string.
    - value: `string` - The string to display as the virtual cursor.

### `ui.list.scrolloff`

The minimum number of visible rows to keep above and below the selected commit where possible.

- type: `u16`
- default: `0`

### `ui.list.columns`

The order and visibility of columns in the commit list.

- type: `array of strings` (enum)
- default: `["graph", "marker", "subject", "name", "hash", "date"]`
- possible values:
  - `graph`
  - `marker`
  - `subject`
  - `name`
  - `hash`
  - `date`

### `ui.list.subject_min_width`

The minimum width of a subject in the commit list.

- type: `u16`
- default: `20`

### `ui.list.date_format`

The date format of a author date in the commit list.

- type: `string`
- default: `"%Y-%m-%d"`

The format must be specified in strftime format.
https://docs.rs/chrono/latest/chrono/format/strftime/index.html

### `ui.list.date_width`

The width of a author date in the commit list.

- type: `u16`
- default: `10`

### `ui.list.date_local`

Whether to show a author date in the commit list in local timezone.

- type: `boolean`
- default: `true`

### `ui.list.name_width`

The width of a author name in the commit list.

- type: `u16`
- default: `20`

### `ui.detail.height`

The height of a commit detail area.

- type: `u16`
- default: `20`

### `ui.detail.date_format`

The date format of a author/committer date in the commit detail.

- type: `string`
- default: `"%Y-%m-%d %H:%M:%S %z"`

The format must be specified in strftime format.
https://docs.rs/chrono/latest/chrono/format/strftime/index.html

### `ui.detail.date_local`

Whether to show a author/committer date in the commit list in local timezone.

- type: `boolean`
- default: `true`

### `ui.user_command.height`

The height of a user command area.

- type: `u16`
- default: `20`

### `ui.refs.width`

The width of a refs list area.

- type: `u16`
- default: `26`

### `graph.color.branches`

Array of colors used for the commit graph.

- type: `array of strings`
- default:
  - `"#E06C76"`
  - `"#98C379"`
  - `"#E5C07B"`
  - `"#61AFEF"`
  - `"#C678DD"`
  - `"#56B6C2"`

Colors should be specified in the format `#RRGGBB` or `#RRGGBBAA`.

### `graph.color.edge`

Color of the edge surrounding the commit circles in the graph.

- type: `string`
- default: `"#00000000"`

Colors should be specified in the format `#RRGGBB` or `#RRGGBBAA`.

### `graph.color.background`

Background color of the commit graph.

- type: `string`
- default: `"#00000000"`

Colors should be specified in the format `#RRGGBB` or `#RRGGBBAA`.

### `color`

The colors of each element of the application.

Note: Graph colors are specified with `[graph.color]`.

- type: `string`
- default: see the example above

Colors should be specified in one of the following formats:

- ANSI color name
  - `"red"`, `"bright-blue"`, `"light-red"`, `"reset"`, ...
- 8-bit color (256-color) index values
  - `"34"`, `"128"`, `"255"`, ...
- 24-bit true color hex codes
  - `"#abcdef"`, ...

### `keybind`

Key bindings for various actions in the application.

See the separate [Custom Keybindings](../keybindings/custom-keybindings.md) section for details.

## Compatibility names and Plastic detail colors

Serialized `hash`/`list_hash_fg`/`detail_hash_fg` mean the full numeric changeset ID;
`tag` color keys mean labels. `list_head_fg` styles the loaded workspace marker.
Remote-branch and stash settings remain accepted but have no active Plastic refs.
`core.git.mailmap` is inert. No default Git action is implied by these names.

Detail status foregrounds use `detail_file_change_add_fg` for exact `Added`,
`detail_file_change_modify_fg` for `Changed`, `detail_file_change_delete_fg` for
`Deleted`, and `detail_file_change_move_fg` for `Moved`. Unknown statuses remain
neutral and verbatim; paths and revision evidence are not colored or discarded.

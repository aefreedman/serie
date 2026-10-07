# Compatibility

The Plastic port provides Unicode `--protocol text` on ordinary terminals,
including Windows, without image support. Auto uses text unless Kitty/iTerm is
detected. Text-cell construction has automated coverage, but interactive Windows
terminal and native macOS/Linux execution remain unverified. A node occupies one
cell, so adjacent nodes may appear separated; terminal/font behavior varies.

The image-protocol tables below are inherited upstream reports, not newly
verified port coverage. Terminal graphics remain deferred for live validation.

## Supported terminal emulators

These image protocols are supported:

- [Inline Images Protocol (iTerm2)](https://iterm2.com/documentation-images.html)
- [Terminal graphics protocol (kitty)](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
  - Supports both the existing graphics protocol mode and [the Unicode placeholder](https://sw.kovidgoyal.net/kitty/graphics-protocol/#unicode-placeholders) mode.

The terminals reported by upstream to work are listed below.

### Inline Images Protocol

| Terminal emulator                                                                   | Note                                                                                                                                         |
| ----------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| [iTerm2](https://iterm2.com)                                                        | But slower than other terminals                                                                                                              |
| [WezTerm](https://wezfurlong.org/wezterm/)                                          |                                                                                                                                              |
| [Rio](https://rioterm.com)                                                          |                                                                                                                                              |
| [VSCode integrated terminal](https://code.visualstudio.com/docs/terminal/basics) \* | Requires the [`terminal.integrated.enableImages` setting](https://code.visualstudio.com/docs/terminal/advanced#_image-support) to be enabled |

\*Not only the VSCode integrated terminal, but any terminal emulator using [xterm.js](https://xtermjs.org) may basically work in the same way as long as [image display feature is enabled](https://github.com/xtermjs/xterm.js/tree/master/addons/addon-image).

### Terminal graphics protocol

| Terminal emulator                         | Unicode placeholder | Note |
| ----------------------------------------- | ------------------- | ---- |
| [kitty](https://sw.kovidgoyal.net/kitty/) | ○                  |      |
| [Ghostty](https://ghostty.org)            | ○                  |      |

Rendering using Unicode Placeholder is available by explicitly specifying `kitty-unicode` as `protocol` option or config.

### Partially supported environments

- tmux is supported only when using the kitty Unicode placeholder protocol.
  - Requires `set -g allow-passthrough on` in tmux.conf (version 3.2+).

### Unsupported environments

- Sixel graphics is not supported.
- Other terminal multiplexers (screen, Zellij, etc.) other than those listed in [Partially supported environments](#partially-supported-environments) are not supported.
- Upstream reported Windows image limitations in [the related issue](https://github.com/lusingander/serie/issues/147#issuecomment-4192875627); this does not prohibit the port's text mode.

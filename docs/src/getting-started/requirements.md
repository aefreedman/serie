# Requirements

- Trusted installed `cm` on PATH, authenticated for the workspace repository.
  Windows validation used cm 11.0.16.10371; native macOS/Linux runs remain unverified.
- Rust 1.88+ to build this local port. On Debian/Ubuntu, building the clipboard
  dependency requires `pkg-config` and `libwayland-dev`.
- An interactive terminal for the TUI; `--protocol text` needs no image support.
  See [Compatibility](./compatibility.md) for rendering limitations.
- Auto clipboard needs a desktop session (X11/Wayland on Linux); initialization
  is lazy. Headless `--dump` needs no terminal or clipboard.

Git is not a production runtime requirement; inherited tests use it in temporary
repositories. Explicit custom commands require their own executables.

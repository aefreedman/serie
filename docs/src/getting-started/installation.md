# Installation

## Local Plastic port

Build from this checkout with Rust 1.88+: `cargo build --locked` (or add
`--release`). Run `./target/debug/serie --workspace "/path/to/workspace" --protocol
text`; on Windows use `.\target\debug\serie.exe`. A trusted authenticated `cm`
on PATH is required. See [Requirements](./requirements.md).

The package/release instructions below install **upstream Git Serie**, not this
local Plastic port. They are retained as upstream reference only.

### [Cargo](https://crates.io/crates/serie)

```
$ cargo install --locked serie
```

### [Arch Linux](https://archlinux.org/packages/extra/x86_64/serie/)

```
$ pacman -S serie
```

### [Homebrew](https://formulae.brew.sh/formula/serie)

```
$ brew install serie
```

or from [tap](https://github.com/lusingander/homebrew-tap/blob/master/serie.rb):

```
$ brew install lusingander/tap/serie
```

### [NetBSD](https://pkgsrc.se/devel/serie)

```
$ pkgin install serie
```

### Downloading binary

You can download pre-compiled binaries from [releases](https://github.com/lusingander/serie/releases).

### Build from source

If you want to check the latest development version, build from source:

```
$ git clone https://github.com/lusingander/serie.git
$ cd serie
$ cargo build --release # Unless it's a release build, it's very slow.
$ ./target/release/serie
```

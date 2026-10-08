# Serie Plastic 0.9.3-plastic.1

Windows-first preliminary release of the read-only Plastic SCM / Unity Version
Control history browser. Adapted from upstream Serie 0.9.3; the binary/package
name remains `serie`, with upstream authorship and MIT license retained.

## Scope and evidence

- History graphs, metadata, changed-file lists, branches/labels, search, refresh,
  qualified-selector clipboard copying, and bounded headless `--dump` inspection.
- The operator reports a functional interactive Windows tool. This is user
  evidence, not exhaustive terminal/graphics-protocol validation.
- Windows x64 is the only initial binary target. macOS/Linux runtime is
  unverified; source builds there are experimental. The native CI matrix is
  prospective coverage, not a claim that GitHub CI or Unix runtime checks ran.
- Built-in file/changeset diffs, including binary diffs, are intentionally
  excluded. Use Plastic's GUI for comparisons; no diff command is configured.
- The built-in backend is read-only. Explicit trusted user-command or external
  clipboard configuration may have effects; no user commands are configured
  by default. `cm` must already be installed, trusted, and authenticated.

## Windows x64 package

The prospective `v0.9.3-plastic.1` draft prerelease packages
`serie-0.9.3-plastic.1-x86_64-pc-windows-msvc.zip` with `serie.exe`, `LICENSE`,
`README.md`, and these notes. The adjacent `.zip.sha256` file contains its SHA-256
hash. Verify before extracting:

```powershell
Get-FileHash -Algorithm SHA256 .\serie-0.9.3-plastic.1-x86_64-pc-windows-msvc.zip
Get-Content .\serie-0.9.3-plastic.1-x86_64-pc-windows-msvc.zip.sha256
```

Compare the hashes (case-insensitive), then extract to a separate directory and
run `serie.exe --version` or `serie.exe --help`. Follow README for workspace
selection and text protocol startup. No installer, `cm`, credentials, user
configuration, or launcher modification is included.

## Known boundaries

Single qualified repository; bounded query windows can omit graph endpoints.
Only explicit primary parents and ordinary merge links form graph ancestry;
other integration types are disclosed rather than invented as edges. Sequential
queries are not a transactional snapshot. Trusted `cm` is assumed; direct child
execution is bounded but hostile process-tree containment is not provided.
Text-cell edge continuity and terminal/font presentation have known limitations.
See README for details and configuration safety boundaries.

This preparation does not publish a release, create a tag, or publish to Cargo.
The tag-triggered workflow creates a draft prerelease marked not latest;
publication requires separate operator approval.

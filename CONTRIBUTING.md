# Contributing

Thanks for contributing to blippy.

## Local Setup

- Rust `1.93+` recommended
- GitHub CLI (`gh`) strongly recommended
- Clone and run:

```bash
cargo run --release
```

## Build, Test, Lint

```bash
cargo build --release
cargo test
cargo clippy --all-targets --all-features
```

Format before opening a PR:

```bash
cargo fmt
```

## Project Conventions

- Use conventional commits (`feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, `perf`)
- Keep changes scoped and focused
- Avoid unnecessary comments when code can be made self-explanatory

## Pull Requests

- Keep PRs small and focused
- Explain what problem is being solved and why the change fixes it
- Link related issues in the PR description
- Add or update tests for behavior changes

## Releases and Changelog

- Releases are automated via GitHub Actions (`release-plz` + `cargo-dist`)
- `CHANGELOG.md` is maintained by the release workflow
- Do not manually hand-edit changelog entries for normal feature work

## Docs to Update with Feature Changes

When behavior changes, update:

- `README.md` for user-facing overview/install flow
- `FEATURES.md` for capability-level changes
- `KEYBINDS.md` for keyboard behavior updates
- `AUTH.md` if authentication behavior changes

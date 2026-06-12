# Development

EndlessTokens uses a Rust workspace and a pinned `mise` toolchain. The workspace
currently tests against Rust 1.95.

## Setup

```sh
mise trust
mise install
```

In sandboxed automation where interactive trust is not appropriate, set
`MISE_TRUSTED_CONFIG_PATHS=$PWD` for the command being run.

## Common Commands

```sh
cargo run -p eltk -- --version
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Prefer the `mise` tasks for normal development because they pin the tool
versions used by local CI:

```sh
mise run check
mise run test
mise run ci
```

## Task Reference

| Task | Purpose |
| --- | --- |
| `fmt` | Check Rust formatting |
| `check` | Run fast Rust type checks |
| `clippy` | Run Rust lints with warnings denied |
| `test` | Run all Rust tests |
| `coverage` | Run tests with a coverage summary |
| `workflow-lint` | Lint GitHub Actions workflows when present |
| `notices` | Check third-party notices against Cargo.lock |
| `audit` | Scan Rust dependencies for security advisories |
| `secret-scan` | Scan committed history for leaked secrets |
| `security-scan` | Scan the filesystem for high-severity findings |
| `build` | Build the release binary |
| `ci` | Run the main local CI task graph |

The `audit` and `security-scan` tasks use ignored caches under `target/`.
Refreshing those databases requires network access.

## Continuous Integration

GitHub Actions runs `mise run ci` on pull requests and pushes to `main`.
Dependabot opens weekly grouped updates for GitHub Actions and Cargo
dependencies.

## Local Artifacts

Do not commit `target/`, `.anvilkit/`, scanner caches, local databases, real
agent transcripts, or one-off review notes. Keep committed fixtures sanitized
and deterministic.

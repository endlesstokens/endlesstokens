# Contributing

EndlessTokens is a local-first token usage tracker for AI coding agents. Keep
changes small, testable, and aligned with the existing Rust workspace shape.

## Development Setup

Use the pinned toolchain and task runner:

```sh
mise trust
mise install
mise run ci
```

If you are not using `mise`, run the equivalent Cargo commands documented in
[docs/development.md](docs/development.md).

Security-oriented tasks fetch advisory databases into ignored `target/` caches.
They are developer-invoked checks, not product runtime behavior.

## Project Conventions

- Product prose uses `EndlessTokens`; crates and the CLI use the `eltk` prefix.
- License new source files with `SPDX-License-Identifier: MIT`.
- Keep `eltk-core` tool-agnostic. Agent-specific behavior belongs in adapters.
- Do not add telemetry, update checks, hosted sync, or pricing refreshes without
  explicit opt-in behavior.
- Keep user data portable. SQLite storage and export/import paths should remain
  inspectable and recoverable.
- Do not commit generated caches, local databases, transcripts, `.anvilkit/`
  artifacts, or private local paths.

## Parser Work

Claude Code is the first adapter, but it is not the architecture. For Claude
Code transcript handling:

- Count only assistant rows with `message.usage`.
- Deduplicate by `(agent, message.id, requestId)`.
- Keep the largest known token total per `requestId` for streamed growth.
- Skip synthetic rows and API error messages.
- Store the canonical `cwd` from each transcript row.

Use sanitized fixtures for parser tests. Fixtures must not contain real
transcript content, secrets, user names, private paths, or machine-specific
state.

## Commits And Pull Requests

Commit subjects follow the Conventional Commit-style format documented in
[README.md](README.md). Keep subjects imperative and under 72 characters.

Before opening a pull request:

```sh
mise run ci
```

Mention any skipped checks and why.

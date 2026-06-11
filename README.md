# EndlessTokens

Your token usage, forever.

## Development

See [docs/development.md](docs/development.md) for the full local setup.

Run the initial CLI smoke check:

```sh
cargo run -p eltk -- --version
```

Run the workspace checks:

```sh
cargo check --all-targets
cargo test --all-targets
```

Run the full local CI task:

```sh
mise run ci
```

An example future configuration lives in
[config.example.toml](config.example.toml).

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidance and
[SECURITY.md](SECURITY.md) for vulnerability reporting.

## Prior Art

See [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) for related projects and
credits.

## Commit Messages

Use Conventional Commit-style subjects:

```text
<type>: <imperative summary>
```

Common prefixes:

- `feat`: user-facing features
- `fix`: bug fixes
- `docs`: documentation and repo guidance
- `ci`: CI and release automation
- `build`: build system, packaging, and dependency tooling
- `deps`: dependency updates
- `docker`: Docker image and base image updates
- `test`: tests and test infrastructure
- `refactor`: behavior-preserving code changes
- `chore`: repository maintenance
- `perf`: performance improvements

## License

EndlessTokens is licensed under the MIT License. See [LICENSE](LICENSE).

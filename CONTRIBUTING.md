# Contributing to GraphDesk

Keep changes focused and discuss consequential storage, security, dependency or
architecture changes before implementing them. Avoid speculative frameworks and
editor redesigns. Follow the [README](README.md) and platform prerequisite guides.
Use pinned toolchains and lockfiles; add regression tests for behavior changes.
Report precisely which OS/WebView/toolchain you actually tested.

Preserve local-first storage, IPC ACLs, session guards and transaction boundaries.
Never edit applied migration SQL: add the next migration. Test rollback with
disposable fixtures. Keep CPU/memory costs bounded; measure before adding caches,
workers or virtualization. Preserve native input/composition and keyboard access.

Before submitting, review the diff for unrelated changes and private data. Keep
dependencies, generated files, databases and test artifacts out of Git. Describe
the problem, solution, checks run and limitations. Update affected documentation
without making unverified platform-support claims. See [testing](docs/testing.md).

## Reports

Include version/commit, OS/architecture, WebView version where known, minimal
reproduction, expected/actual results and sanitized errors. Linux reports should
identify compositor and Wayland versus XWayland when known. Never attach personal
graphs, credentials, private paths or system configuration dumps. No dedicated
private security-reporting channel is currently documented; ask maintainers for a
private channel before posting sensitive details publicly.

## License

The project is [MIT-licensed](LICENSE). Submit only work you have the right to
contribute under that license. Review dependency licenses before adding or
redistributing them.

# Development Rules

These rules apply to all development agents working in this repository.

## Scope and workflow

- Inspect Git status, branch, upstream, and relevant code before changing files.
- Current remote `main` is authoritative. Never restore discarded PoCs without an explicit request.
- Preserve unrelated user changes. Respect analysis-only requests and approval gates.
- `main` is the only permanent branch. Start each task from updated `main`.
- Use `feat/`, `fix/`, `refactor/`, `docs/`, `test/`, or `chore/` branches.
- Use English Conventional Commits: `type(scope): imperative description`.
- Keep one principal feature or fix per PR. Target `main`; prefer squash merge.
- Never push directly to `main`. Any push requires explicit user authorization.
- Never merge without explicit approval and passing required checks.
- Start subsequent work from updated `main` after the preceding PR is integrated.

## Architecture and Rust

- Keep one crate unless a concrete boundary justifies another.
- Keep `domain` independent of Iced, operating systems, files, and IPC.
- Put use-case coordination in `application`, rendering and input translation in
  `presentation`, and platform APIs, persistence, and IPC in `infrastructure`.
- Platform adapters depend on application/domain types, not the reverse.
- Apply SOLID where it addresses observed coupling or responsibility problems.
- Prefer concrete types, functions, and enums. Introduce traits only for a real
  substitution or testing boundary. Avoid speculative layers and duplicate state.
- Reuse existing code. Refactor only what the current increment requires.
- Propagate recoverable errors with `Result` and useful context. Avoid
  `unwrap`/`expect` on runtime input and platform operations; justify intentional panics.
- Avoid `unsafe`; document safety invariants and verification when unavoidable.
- Keep dependencies minimal; review maintenance, licenses, and known advisories.
- Commit `Cargo.lock`. Use `--locked` in CI. Claim an MSRV only after testing it.
- Keep rustfmt and Clippy passing without blanket warning suppressions.

## Security, performance, and platforms

- Validate configuration and IPC data, including input sizes, numeric ranges,
  and non-finite values. Never commit secrets or log sensitive data.
- Avoid shell interpolation; use structured APIs or explicit arguments.
- Use platform-standard application directories and atomic persistence writes.
  Recover from missing or invalid files. Keep blocking I/O outside UI updates.
- Prefer event-driven input and monitor handling. Do not introduce continuous
  polling without measured necessity, unbounded queues, or per-motion disk writes.
- Schedule animation work only while needed or for a specific deadline.
- Preserve avatar and panel state across surface and monitor changes.
- Measure performance claims against a recorded baseline.
- Linux/Hyprland is the primary runtime target; macOS is the second target.
- Isolate Linux APIs and dependencies with target configuration. Keep the fallback
  build working with `--no-default-features`.
- Keep shared positioning policy free of platform handles. Distinguish logical
  coordinates, physical pixels, scale, and output identity.
- Document compositor-specific limitations. Do not claim native macOS behavior
  from compilation or cross-monitor dragging without reproducible runtime evidence.

## Verification and delivery

- Add focused, deterministic tests for new non-trivial logic and fixed regressions.
- Run `cargo fmt --all --check`, plus Clippy, tests, and build for affected configurations:

  ```sh
  cargo clippy --locked --all-targets -- -D warnings
  cargo test --locked --all-targets
  cargo build --locked
  ```

- Repeat the Cargo checks with `--no-default-features` for the Linux fallback and
  macOS. CI compilation does not replace native GUI/compositor validation.
- Report exact checks and results, including checks not run and platform limits.
- PR descriptions must explain the problem, resulting behavior, validation, and risks.
- Update documentation when behavior or compatibility changes. Stop when acceptance
  criteria are met; do not add unrelated cleanup.

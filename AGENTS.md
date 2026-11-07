# AGENTS.md — rules for any agent working in this repository

## Build & verification policy (owner's rule, 2026-11-07)

**Do NOT build locally.** Never run `cargo build`, `cargo check`,
`cargo test`, `cargo clippy` or any other command that compiles this
project on the local machine, unless the user has explicitly approved
that specific local build first.

Local builds are heavy (multi-GB `target/`, long CPU) and the owner
does not want them. **GitHub Actions CI is the only build and test
environment.**

### The loop

1. Make your edits.
2. Commit and push to `master`.
3. Read the result of the **Build, Test & Binary** workflow
   (`gh run list`, `gh run view <id> --log-failed`).
4. If CI fails, fix the problem from the CI logs and push again.
5. Repeat until CI is green. Only then call the work verified.

### What is allowed locally (no approval needed)

- Reading and editing source files.
- `grep` / `sed` / `rg` searches and other non-compiling inspection.
- `git` operations (add, commit, push, status, log).
- `gh` commands (run status, logs, repo info).

### Exceptions

- A local build is allowed only when the user says so, explicitly,
  for that run. Approval for one build is not approval for the next.
- If a problem genuinely cannot be diagnosed from CI logs, stop and
  ask the user — do not quietly fall back to a local build.

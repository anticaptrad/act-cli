# Agent instructions

## Scope

This repository contains the AntiCapTrad operator CLI. Keep it headless,
scriptable, deterministic, and safe for CI use.

## Security invariants

- Never accept access tokens, provider keys, stream keys, or secrets as command
  arguments because process listings and shell history expose them.
- Read the platform access token from `ACT_ACCESS_TOKEN` only when a command
  requires authentication.
- Reject redirects so credentials cannot be forwarded to another origin.
- Require HTTPS for non-loopback API endpoints.
- Never print authorization headers, raw provider responses, or secret-bearing
  URLs in errors.
- Mutating commands must use explicit confirmation and idempotency keys when
  they are introduced.

## Quality gates

Run `cargo fmt --check`, `cargo test --locked`, and
`cargo clippy --all-targets --locked -- -D warnings` before publishing changes.

Do not rewrite unrelated user changes or use destructive Git commands.

## Repository-local Git worktrees

- Create or use a Git worktree only when the human operator explicitly authorizes it for the current task. Concurrency or a dirty checkout is not permission by itself.
- Put every authorized worktree at `<repository-root>/tmp/worktrees/<name>`; from the repository root, use `./tmp/worktrees/<name>`. Never place worktrees beside repositories or organization directories.
- Keep `tmp`, `temp`, `tmp/worktrees`, and `temp/worktrees` ignored in the repository-root `.gitignore`. Do not commit files from those directories.
- Relocate or remove a worktree only when the operator explicitly requests it. Before removal, preserve and publish intended changes, verify its commit is represented on the target branch, and confirm there are no tracked, untracked, ignored-sensitive, or in-use files that must survive. Remove it with `git worktree remove <path>` without `--force`; never delete a worktree directory with `rm`.

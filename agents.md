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

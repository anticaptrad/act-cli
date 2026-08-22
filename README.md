# AntiCapTrad CLI

`act-cli` is the headless operator and automation client for the AntiCapTrad
publishing platform. It complements the native Rust and Flutter applications
without requiring a graphical session.

The first vertical slice provides liveness, readiness, principal, and arbitrary
read-only JSON queries. Provider and stream mutations will be added only with
the platform's canonical interface schemas, explicit confirmation, and
idempotency-key support.

## Install and run

```sh
cargo install --path .
act config
act health
act ready
```

Configure a deployed API:

```sh
export ACT_API_URL=https://api.anticaptrad.example
act health
```

Authenticated commands read the bearer token from the environment; tokens are
not accepted as command arguments:

```sh
export ACT_ACCESS_TOKEN='short-lived-platform-token'
act me
act get --authenticated /api/providers
```

Every command emits JSON to standard output. Use `--compact` for JSON Lines and
automation. Errors go to standard error and produce a non-zero exit code.

## Security properties

- non-loopback endpoints require HTTPS;
- URL credentials and fragments are rejected;
- HTTP redirects are rejected instead of forwarding authorization;
- connection and total request timeouts are bounded;
- response bodies are capped at 1 MiB;
- API responses must be JSON;
- no provider secret or access token is written to logs or output.

## Quality gates

```sh
cargo fmt --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
```

Licensed under the MIT License.

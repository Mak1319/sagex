# Copilot — Rebuild in `crates2/`, DO NOT USE `crates/`

Read `@problem.txt` first — it is the binding spec.

The root `crates/` directory is FORBIDDEN (frozen legacy).

- Do not read, list, glob, grep, or open any file under `crates/`.
- Do not import, depend on, link, or reference `crates/*` from new code.
- Do not create, edit, or move files under `crates/`.
- Do not use root `Cargo.toml [workspace] members`, `Cargo.lock`, or `target/` as reference.
- Do not copy-paste code, configs, or TOML node files from `crates/`.

Do: build the variation from scratch in `crates2/` only (`crates2/<crate>/src/...`, own workspace). Do not edit root `Cargo.toml`.

Follow `@problem.txt`: invisible per-session watermark, recipient PQC signature, offline DLT, extract → lookup → verified attribution.

Low-memory debug: scope to one crate — `cargo check -p <crate>`, `cargo test -p <crate> <single> -- --exact`, `CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true`. Never full build/test. One cargo process at a time.

Tests: never inline `#[cfg(test)]` in `src/`. All tests in `crates2/<crate>/tests/<topic>.rs` via the public API. No throwaway test/scratch files at root or in `src/`.

No tests without being asked: never write, add, or modify tests on your own initiative. Verify with `cargo check -p <crate>` only, unless the user explicitly asks for tests.

compiled-service: compile first (`cargo build -p <svc>`), start the produced binary — never `cargo run`/watchers. One isolated env per service via `run-isolate.sh` (`environments/<svc>`); GUI services add `GUI=1`. TOML-configurable (own structure + `.toml.example`, never commit real configs). Env files persist across boots; delete only on user order. All envs under gitignored `environments/`.

example-test-set: fake synthetic dataset linked 1:1 to a service, mimicking user artifacts for end-to-end exercise without real user data. Lives at `environments/<service>/example-test-set/` (gitignored). Rules: `SYNTHETIC` marker + obviously-fake identities; deterministic seed in `MANIFEST`, persists across boots (regen only on user order); mirrors exactly what the linked service consumes/produces.

See `AGENTS.md` for the binding rules.

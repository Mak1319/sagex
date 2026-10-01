# CLAUDE — Rebuild in `crates2/`, DO NOT USE `crates/`

Read `@problem.txt` first — it is the binding spec.

**Root `crates/` is FORBIDDEN and FROZEN (legacy).**

- NEVER read, list, glob, grep, or open `crates/**`.
- NEVER import, depend on, or reference `crates/*` from new code.
- NEVER create or edit files under `crates/`.
- NEVER use root `Cargo.toml [workspace]`, `Cargo.lock`, `target/` as reference.
- NEVER copy-paste from `crates/`.

**DO: build from scratch in `crates2/` only** (`crates2/<crate>/src/...`, own workspace `Cargo.toml`). Do NOT touch root `Cargo.toml` members.

**Follow `@problem.txt`:** per-session invisible watermark at decrypt, recipient PQC signature (own private key, e.g. ML-DSA), offline DLT ledger, extract → lookup → verified attribution. Offline/air-gapped only.

**Low-memory debug (mandatory):** one crate at a time — `cargo check -p <crate>`, `cargo test -p <crate> <single> -- --exact`, `CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true`. Never full `cargo build/test`. No parallel cargo procs. Read files in slices, never dump `target/`/`Cargo.lock`/logs.

**Tests:** NEVER inline `#[cfg(test)]` in `src/`. All tests in `crates2/<crate>/tests/<topic>.rs` via public API. No throwaway test files at root or in `src/`.

**No tests without being asked:** NEVER write, add, or modify tests on your own initiative — no quick tests, no verification tests, no scaffolding. Tests waste time/tokens. Verify with `cargo check -p <crate>` only, unless the user explicitly asks for tests.

**compiled-service:** compile first (`cargo build -p <svc>`), then start the produced binary — never `cargo run`/watchers. One isolated env per service via `run-isolate.sh` (`BINARY_PATH=target/debug/<svc> ./run-isolate.sh environments/<svc>`); GUI services add `GUI=1` (window-capable binds, flatpak-level tradeoff). Rules: TOML-configurable (own structure + tracked `.toml.example`, never commit real configs); env files persist across boots (delete only on user order); all envs under gitignored `environments/<service>/`.

**example-test-set:** fake synthetic dataset linked 1:1 to a service, mimicking user artifacts to exercise it end-to-end without real user data. Lives at `environments/<service>/example-test-set/` (gitignored). Rules: `SYNTHETIC` marker file + obviously-fake identities (never real data, generate don't reuse); deterministic from a seed recorded in `MANIFEST`, persists across boots (regen only on user order); must mirror exactly what the linked service consumes/produces.

See `AGENTS.md` — same rules, binding.

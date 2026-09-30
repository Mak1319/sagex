# AGENTS — Rebuild in `crates2/`, DO NOT USE `crates/`

> **Hard rules for all agents (human or AI). Read `problem.txt` first — it is the spec.**

<!--
## 1. FORBIDDEN: root `crates/` (frozen legacy)

- DO NOT read, list, `glob`, `grep`, `cat`, or open any file under `crates/`.
- DO NOT import, depend on, link, or reference `crates/*` from any new code.
- DO NOT create, edit, or move files under `crates/`.
- DO NOT use the commented-out `crates/*` entries in root `Cargo.toml`, stale `Cargo.lock` history, or old `target/` artifacts as reference — they describe the removed legacy build.
- DO NOT copy-paste code, configs, or node `.toml` files from `crates/`.
- If a tool auto-suggests or auto-reads `crates/`, STOP, ignore it, continue without it.
- When in doubt: pretend `crates/` does not exist.-->

## 2. DO: build variation in `crates2/`

- Build the new approach **from scratch** in top-level `crates2/` only. Example: `crates2/<new-crate>/src/...`.
- Do NOT create `packages/`, `apps/`, `v2/` — the target dir is `crates2/`.
- Root `Cargo.toml [workspace] members` lists `crates2/*` only. Do NOT re-add `crates/*`. There is no nested `crates2/Cargo.toml` workspace — keep it that way.
- Keep each `crates2/*` crate small, single-purpose, offline-only, no cloud KMS, no public chain.

## 3. MUST follow `@problem.txt` (binding spec)

- `@problem.txt` overrides any guess: broadcast-encrypt / individually-decrypt, per-recipient+per-session invisible forensic watermark at decrypt time, visually identical but forensically distinct copies.
- Each decrypt event: recipient signs record with their own NIST PQC private signing key (e.g. ML-DSA), commit to offline tamper-evident DLT, no single-admin rewrite/delete.
- Must support: watermark extract from leaked copy → ledger lookup → cryptographically verified attribution record.
- PQC for KEX + signatures, air-gapped, no external/cloud dependencies.

## 4. Low-memory debug (mandatory)

Agents + humans debugging `crates2/` MUST minimize RAM:

- NEVER run full-workspace `cargo build / test / clippy`. Scope everything: `cargo check -p <crate>`, `cargo test -p <crate> --lib`, one crate at a time.
- Prefer `cargo check` over `cargo build`; prefer `--message-format=short`, run single test: `cargo test -p <crate> <single_test> -- --exact --nocapture`.
- Limit parallelism: `CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true cargo check -p <crate>`. Do not raise jobs.
- Do NOT `cat` big files, `target/`, `Cargo.lock`, logs. Use `head -n`, `rg --max-count`, read in slices. Close files immediately.
- Do NOT run multiple cargo processes / watchers / servers in parallel. One command at a time, `cargo clean -p <crate>` if RAM spikes.
- Reuse sandbox runner pattern (`run-isolate.sh` / bwrap) for executing test binaries instead of holding debug servers in memory.

## 5. Tests live in `tests/` — never inline, never ad-hoc files

- NEVER put `#[cfg(test)] mod tests` inside `crates2/*/src/*.rs`. All tests go in the crate's `tests/` dir as integration tests (e.g. `crates2/<crate>/tests/<topic>.rs`) using the public API.
- NEVER create throwaway test/scratch files at repo root or in `src/` (no `test_*.rs`, `scratch.rs`, `tmp_*`, root-level `*.rs` experiments). If you need a repro, add it as a proper `tests/<name>.rs` file or delete it before finishing.
- One topic per test file (`aes_postcard.rs`, `kem.rs`, `dsa.rs`, …). Run one file/test at a time: `cargo test -p <crate> --test <file> <name> -- --exact --nocapture`.

## 6. DO NOT write tests unless the user explicitly asks

- NEVER write, add, or modify any test (in `tests/`, inline, or anywhere) on your own initiative. No "add a quick test", no "verify with a test", no test scaffolding.
- Reason: unrequested tests waste time and tokens (slow crypto tests, extra compiles, extra review).
- Verify with `cargo check -p <crate>` only, unless the user asked for tests.
- When the user DOES ask for tests, §5 applies (proper `tests/` dir, one topic per file).

## 7. Terminology: `compiled-service` (+ binding rules)

- **Definition**: a `compiled-service` is a service that MUST be compiled first from source (`cargo build -p <svc>` — scoped, one crate, `CARGO_BUILD_JOBS=1`), and then the produced binary (`target/debug/<svc>`) is what gets started as the service. Never run services via `cargo run` / watchers / long-lived dev servers.
- **Isolation**: every `compiled-service` gets its own environment. Isolate with `run-isolate.sh` logic (bwrap: system dirs read-only, env dir bound as `HOME`, namespaces unshared, one process at a time). Usage: `BINARY_PATH=target/debug/<svc> ./run-isolate.sh environments/<svc> [args...]`. GUI services add `GUI=1` (`GUI=1 BINARY_PATH=target/debug/<gui> ./run-isolate.sh environments/<gui>`), which binds the minimum for a window (Wayland/X11 sockets, Xauthority, GPU, fonts, session bus) — accepted tradeoff, roughly flatpak-level isolation; headless services stay fully locked down.
- **Rule 1 — TOML config**: every `compiled-service` MUST be configurable through TOML. Services do NOT need the same config structure — each defines its own (e.g. `environments/<svc>/config.toml`). Ship a tracked `<svc>.toml.example` template next to the crate; NEVER commit real configs with secrets/keysets.
- **Rule 2 — persistence**: files a service produces in its test env (DBs, blobs, logs, sealed material) MUST persist across boots/restarts. NEVER wipe, reseed, or recreate env contents on boot. Delete/regen env data ONLY when the user explicitly says so.
- **Rule 3 — location + ignore**: all test envs live under top-level `environments/<service>/` (create with `mkdir -p` at runtime). Everything under `environments/` is gitignored — never force-add env files.

## 8. Terminology: `example-test-set` (linked to services)

- **Definition**: an `example-test-set` is a fake, synthetic dataset generated to mimic real user artifacts (usernames, `.prv`/`.pub` key files, documents, ledger records, sealed blobs) so a `compiled-service` can be exercised end-to-end in its test env without touching real user data. Each `example-test-set` is linked 1:1 to one service.
- **Rule 1 — location**: lives inside the service's env at `environments/<service>/example-test-set/`. Gitignored via `environments/` — never force-add, never move outside.
- **Rule 2 — clearly fake**: every `example-test-set` MUST contain a top-level `SYNTHETIC` marker file stating the data is generated and fake, and fake identities MUST be obviously synthetic (e.g. `alice-test`, `bob-test` — never real usernames). NEVER copy real user files into a test-set; generate, don't reuse.
- **Rule 3 — deterministic + stable**: generate from a fixed seed recorded in `environments/<service>/example-test-set/MANIFEST` (seed + generator version + item list) so runs are reproducible. NEVER regenerate or wipe an existing test-set on boot/service start — it persists like all env data (§7 Rule 2). Regen/delete ONLY when the user explicitly says so.
- **Rule 4 — service-shaped**: the test-set MUST mirror exactly what the linked service consumes/produces (same file names, formats, TOML keys the service reads). If the service's format changes, its linked `example-test-set` generator changes with it — never drift.

## 9. FORBIDDEN: `crates2/` (do not inspect)

- DO NOT read, list, `glob`, `grep`, `cat`, or open any file under `crates2/`.
- DO NOT import, depend on, link, or reference `crates2/*` from any new code.
- DO NOT create, edit, or move files under `crates2/`.
- If a tool auto-suggests or auto-reads `crates2/`, STOP, ignore it, continue without it.
- When in doubt: pretend `crates2/` does not exist.

Violation of §1 or ignoring `problem.txt` defeats the variation task.

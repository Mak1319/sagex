# SAGEX — Video Script (5-6 min, Hackathon Judges, Desktop-GUI Demo)

**Product:** `sagex` — offline post-quantum forensic attribution for Office documents.
Same plaintext for everyone, forensically distinct copy per recipient + PBFT ledger proof.

**Core loop:** `seal (sender)` -> `open_verify -> derive wm-id -> register-gate -> decrypt -> embed` -> `leak -> extract -> ledger lookup -> verify`
(`crates/sagex-forensic/src/envelope.rs`, `watermark.rs`, `doc.rs`, `crates/sagex-ledger/src/model.rs|pbft.rs`, `crates/sagex-gateway/src/api.rs`)

---

## Pre-Recording Setup

```bash
./scripts/build.sh
./scripts/start.sh  # ledger 7000-7003, gateway 127.0.0.1:8081, certauth 127.0.0.1:8082, chatsrv :8080
cargo run -p office-watermark-ui
SAGEX_GATEWAY_URL=http://127.0.0.1:8081 cargo run -p sagex-ledger-ui
```

**Assets ready:** `report.docx`, `report-wm.docx` (watermarked), `leaked.docx` (watermarked copy).

---

## SCENE 0 — HOOK [0:00-0:35]

**VISUAL:** 3 copies of same `report.docx` labeled Alice / Bob / Carol -> one `leaked.docx` on a leak site. Zoom on question: WHO LEAKED?

**VOICEOVER:**
> "One confidential file. Three authorized people. One leak on the internet."
> "Every copy looks identical. Access logs can be edited by an admin. Old watermarks are identical for everyone — so they blame everyone, and prove nothing."
> "This is the exact problem we were asked to solve. This is SAGEX."

**ON-SCREEN:** `Broadcast-encrypt / Individually-decrypt -> no attribution`

---

## SCENE 1 — SOLUTION IN 30 SEC [0:35-1:20]

**VISUAL:** Simple 5-box diagram. Highlight each as spoken.

**VOICEOVER:**
> "SAGEX seals a document once, then every decrypt creates a unique, invisible forensic fingerprint for that person and that session."
> "The recipient signs that decrypt event with their own post-quantum private key. That signed record goes to an offline blockchain no single admin can rewrite."
> "When a file leaks, we extract the fingerprint, look it up, and cryptographically prove who decrypted it."
> "All NIST post-quantum: X25519 plus ML-KEM-768 for key exchange, ML-DSA-65 for signatures. Fully offline, air-gapped. No cloud KMS, no public chain."

**ON-SCREEN:** `Seal -> Decrypt+Watermark -> Sign (ML-DSA-65) -> PBFT commit -> Extract+Prove`

---

## SCENE 2 — DEMO A: INVISIBLE WATERMARK [1:20-3:10]

**VISUAL:** `office-watermark-ui` fullscreen. Cursor through 4 tabs.

**VOICEOVER:**
> "Let's fingerprint a real Office file. This is our desktop app — Encode, Decode, Verify, Roundtrip."
> "I pick `report.docx`. I type the session watermark — in production this is auto-derived as `wm-` plus HMAC of recipient key, file hash, user and session — and hit Encode. It suggests `report-wm.docx`. Done: `encoded Docx: N xml parts`."
> "What did it do? For every XML part inside the docx, it injected one invisible attribute: `sagex:wm`. Word, Excel, PowerPoint and LibreOffice ignore it for rendering, but preserve it. Open it — it just opens. Visually identical, forensically distinct. Works for docx, pptx, xlsx, ods, odt, odp."
> "Now Decode the leaked file — we get a per-part table, `entry => value`, with one common verdict."
> "Verify gives us the strict proof: `matched / total, missing: 0`. Tamper one part and it fails. Unmarked file — Decode says no watermark, Verify says not-ok. Never silent."

**SCREEN ACTIONS:**
1. Encode: `report.docx` + `wm-aB12...` -> `report-wm.docx`
2. Open `report-wm.docx` in Word/LibreOffice to show normal rendering
3. Decode `leaked.docx` -> copy `wm-...` value
4. Verify `leaked.docx` -> show pass, then show tampered fail

---

## SCENE 3 — DEMO B: LEAK TO PROOF [3:10-4:40]

**VISUAL:** Switch to `sagex-ledger-ui` — Ledger Explorer. Show `connected` to `http://127.0.0.1:8081`.

**VOICEOVER:**
> "Behind the app, four offline ledger nodes on ports 7000 to 7003 reach PBFT quorum, fronted by our gateway on 8081. One record equals one block."
> "I paste the leaked watermark into Search — By watermark. One click: block number, user ID, session ID, file hash, payload hash, and the recipient's ML-DSA signature."
> "Click the row — the app recomputes the block hash locally from its fields. Green check means proof, not trust."
> "Records tab is the full chain, newest first. Status tab shows all four nodes — height, view, tip — plus gateway outbox. Kill one node, we still commit. That's `f=1` fault tolerance."
> "Logs need an auditor permit — minted offline with `sagex-certauth issue-permit --identity ledger-auditor` — never stored on disk."

**SCREEN ACTIONS:**
1. Search -> `By watermark: wm-...` -> show `bob / sess-1 / file_hash`
2. Click row -> `hash recomputed from fields` (green check)
3. Status -> 4 node cards + `outbox pending: 0`
4. B-roll terminal: `./target/debug/sagex-ledger-node verify --db data/node0.db`

---

## SCENE 4 — WHY TRUST IT [4:40-5:20]

**VISUAL:** Quick split: tamper test + offline badge.

**VOICEOVER:**
> "Three guarantees judges care about. One — tamper fails closed. Flip the watermark flag or one ciphertext byte, signature verification fails before decrypt. No decrypt without register-gate approval, nothing touches disk unmarked."
> "Two — non-repudiation. The record is signed by the recipient's own private key, bound to canonical bytes. They can't deny it."
> "Three — immutability without internet. TCP JSON-lines plus SQLite, PBFT pre-prepare, prepare, commit. No cloud, no public blockchain to depend on."

---

## SCENE 5 — CLOSE [5:20-5:50]

**VISUAL:** Side-by-side: two visually identical docs, different `sagex:wm` in Decode. End card with repo + test command.

**VOICEOVER:**
> "Same document for everyone. Unique invisible truth for each person. Verifiable proof for every leak."
> "SAGEX — post-quantum forensic attribution that runs fully air-gapped. Thank you."

**END CARD:**
```
SAGEX — distributed post-quantum cryptographic attribution
cargo test -p sagex-forensic -p sagex-ledger -p sagex-gateway
ledger 7000-7003 | gateway :8081 | GUIs: office-watermark-ui + ledger-ui
```

---

## Director Notes

- Keep GUI fullscreen 70% of runtime. Terminal only as 5-sec B-roll for `start.sh` + `verify`.
- Record Decode -> Search copy-paste in one continuous take — that's the money shot for attribution.
- Total voiceover: ~800 words at natural pace.

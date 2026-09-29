# office-watermark-ui — desktop GUI for Office watermarks

Native GPUI app with full parity to the `office-watermark-demo` CLI:
encode / decode / verify `sagex:wm` watermarks in Office files, plus a
roundtrip self-test. Same purpose, as a desktop app.

## Run

```bash
cargo run -p office-watermark-ui
```

## Tabs

- **Encode**: pick an input file (`docx/pptx/xlsx/ods/odt/odp`), type the
  watermark text (validated live: non-empty, ≤ 4096 bytes, no illegal XML
  control chars), optionally override the suggested `<stem>-wm.<ext>`
  output path, then Encode. Result: `encoded <format>: N xml parts`.
- **Decode**: pick a watermarked file → per-part `entry => value` table plus
  the common-value verdict (or the distinct-values list on disagreement).
- **Verify**: pick a file → per-part `OK/DIFF` rows plus the strict summary
  `matched/total (missing: M)`; pass only when every part agrees and none
  is missing (same rule as the CLI).
- **Roundtrip**: pick `docx`/`ods` + text → builds the in-memory sample,
  encodes, decodes, shows per-part hits and `roundtrip OK/FAIL`.

All file work runs on a background thread; the UI never blocks. Native
file picker dialogs throughout.

## Layout

- `src/main.rs` — application bootstrap (mirrors `sagex-ledger-ui`).
- `src/app.rs` — `WatermarkApp` view (tabs, pickers, background runs).
- `src/ops.rs` — blocking wrappers over `office-watermark` with plain
  result structs + sample builders (relocated from the demo CLI).

## Tests

```bash
cargo test -p office-watermark-ui
```

Covers text validation, both sample roundtrips, and a full
encode→decode→verify file cycle including tamper detection
(a modified part must fail verify) and the unmarked-file cases.
Headless CI compiles + unit-tests; the window itself needs a display,
so click through all four tabs manually as the final gate.

# sagex_loader

Standalone wizard splash demo for the `sagex_ui` crate. Separate binary,
zero shared code with the main app — two-panel dialog in the Android
Studio splash style: white brand panel (logo, name, version, progress,
status, lock row) beside a full-bleed flat illustration with a small
credit line.

## Run

```bash
cargo run -p sagex_ui --bin sagex_loader
```

No CLI flags, no stdin protocol — just the demo.

## Configure the animation (in code)

Edit `SplashConfig` in `splash.rs`, or override per-field in
`src/bin/loader.rs`:

```rust
let cfg = SplashConfig {
    duration: Duration::from_millis(2600), // 0→100% fill time
    end: SplashEnd::Hold,                  // Hold | Loop | Exit
    hold: Duration::from_millis(900),      // pause on full before Loop/Exit
    bar: 0x00a884,                         // fill color (sagex accent green)
    title: "sagex",                        // wordmark
    version: "sagex // 0.1.0",             // version line
    tagline: "End-to-end encrypted",       // lock row text
    credit: "Post-quantum secure",         // art corner print
};
```

| `end` | Behavior when the bar fills |
|---|---|
| `Hold` (default) | freeze on the finished splash |
| `Loop` | pause `hold`, then replay the fill |
| `Exit` | pause `hold`, then quit with code 0 |

The status line follows the fill (`Connecting… → Restoring session… →
Loading rooms… → Opening chat… → Ready`). Artwork is the hand-drawn
`assets/splash-art.svg` (embedded, no file IO); dialog palette is fixed
white/ink to match the reference.

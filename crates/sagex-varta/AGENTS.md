# AGENTS — `sagex-varta` crate rules (binding)

## 1. DESIGN.md is law

- Every UI change in this crate MUST follow `DESIGN.md` (Blueprint Precision)
  at repo root: tokens, typography (Space Grotesk / Geist / JetBrains Mono),
  8px grid, hairline borders, radii, elevation rules. No invented colors,
  fonts, shadows, or pill shapes.
- Colors ONLY via `crate::theme` (`theme::theme(cx)` tokens). NEVER hardcode
  `rgb()` / `rgba()` / hex in component files. Adding a new token? Add it to
  `src/theme.rs` first, mapped from DESIGN.md.

## 2. gpui-kit components only

- Build UI from `gpui-kit` / `gpui-component` primitives (`Icon`, `Button`,
  `Tab`, `TitleBar::window_options`, etc.). No hand-rolled widgets when a kit
  component exists. Icons via `Icon::empty().path("icons/<name>.svg")`
  (embedded `assets/icons/`) or kit `IconName`.

## 3. RenderOnce components, one dir each

- Every component is `#[derive(IntoElement, Clone)]` + `impl RenderOnce`,
  living in its own dir: `src/components/<name>/mod.rs`, exported from
  `src/components/mod.rs`. No `Render` + `Context` state unless the user
  explicitly asks for interactivity.
- Shared app state (theme, etc.) flows via `cx` globals (`theme::init`),
  never via struct fields that duplicate context.

## 5. Components inherit gpui-base behavior

- Prefer `gpui-base` state/behavior primitives (`gpui_kit::base::*`: `Tabs`,
  `Root`, resizable panels, focus/overlay/virtualization) over hand-rolled
  state machines. Presentation belongs to this crate; behavior belongs to
  the base layer.
- Interactive tab-style controls follow the entity-downgrade pattern:
  parent view owns `selected: usize`, children built via
  `.children(items.enumerate().map(...))` with
  `cx.entity().downgrade()` + `entity.update(cx, |this, cx| { ...; cx.notify(); })`
  in `on_click`. Never recreate entities inside `render`.

## 4. Follow user instructions literally

- Implement EXACTLY what the user asked. Do NOT add extras (badges, chips,
  telemetry, placeholder content) without explicit permission.
- Do NOT rename, restyle, or restructure beyond the request. Window geometry
  (`1000×700`), decorations (`Client`), and asset setup change ONLY on direct
  instruction.
- When ambiguous, ask — do not improvise. State assumptions before acting.

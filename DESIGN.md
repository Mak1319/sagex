---
name: Blueprint Precision
colors:
  surface: '#f8f9ff'
  surface-dim: '#cbdbf5'
  surface-bright: '#f8f9ff'
  surface-container-lowest: '#ffffff'
  surface-container-low: '#eff4ff'
  surface-container: '#e5eeff'
  surface-container-high: '#dce9ff'
  surface-container-highest: '#d3e4fe'
  on-surface: '#0b1c30'
  on-surface-variant: '#434655'
  inverse-surface: '#213145'
  inverse-on-surface: '#eaf1ff'
  outline: '#737686'
  outline-variant: '#c3c6d7'
  surface-tint: '#0053db'
  primary: '#004ac6'
  on-primary: '#ffffff'
  primary-container: '#2563eb'
  on-primary-container: '#eeefff'
  inverse-primary: '#b4c5ff'
  secondary: '#565e74'
  on-secondary: '#ffffff'
  secondary-container: '#dae2fd'
  on-secondary-container: '#5c647a'
  tertiary: '#005b7c'
  on-tertiary: '#ffffff'
  tertiary-container: '#00759f'
  on-tertiary-container: '#e1f2ff'
  error: '#ba1a1a'
  on-error: '#ffffff'
  error-container: '#ffdad6'
  on-error-container: '#93000a'
  primary-fixed: '#dbe1ff'
  primary-fixed-dim: '#b4c5ff'
  on-primary-fixed: '#00174b'
  on-primary-fixed-variant: '#003ea8'
  secondary-fixed: '#dae2fd'
  secondary-fixed-dim: '#bec6e0'
  on-secondary-fixed: '#131b2e'
  on-secondary-fixed-variant: '#3f465c'
  tertiary-fixed: '#c4e7ff'
  tertiary-fixed-dim: '#7bd0ff'
  on-tertiary-fixed: '#001e2c'
  on-tertiary-fixed-variant: '#004c69'
  background: '#f8f9ff'
  on-background: '#0b1c30'
  surface-variant: '#d3e4fe'
typography:
  display-lg:
    fontFamily: Space Grotesk
    fontSize: 40px
    fontWeight: '700'
    lineHeight: 48px
    letterSpacing: -0.03em
  headline-lg:
    fontFamily: Space Grotesk
    fontSize: 30px
    fontWeight: '600'
    lineHeight: 38px
    letterSpacing: -0.02em
  headline-md:
    fontFamily: Space Grotesk
    fontSize: 22px
    fontWeight: '600'
    lineHeight: 30px
    letterSpacing: -0.015em
  headline-sm:
    fontFamily: Space Grotesk
    fontSize: 18px
    fontWeight: '600'
    lineHeight: 26px
    letterSpacing: -0.01em
  body-lg:
    fontFamily: Geist
    fontSize: 16px
    fontWeight: '400'
    lineHeight: 26px
    letterSpacing: -0.005em
  body-md:
    fontFamily: Geist
    fontSize: 14px
    fontWeight: '400'
    lineHeight: 22px
    letterSpacing: '0'
  body-sm:
    fontFamily: Geist
    fontSize: 12px
    fontWeight: '400'
    lineHeight: 18px
    letterSpacing: 0.005em
  code-inline:
    fontFamily: JetBrains Mono
    fontSize: 13px
    fontWeight: '500'
    lineHeight: 20px
    letterSpacing: -0.01em
  label-lg:
    fontFamily: JetBrains Mono
    fontSize: 13px
    fontWeight: '600'
    lineHeight: 18px
    letterSpacing: 0.04em
  label-md:
    fontFamily: JetBrains Mono
    fontSize: 11px
    fontWeight: '500'
    lineHeight: 16px
    letterSpacing: 0.06em
  label-sm:
    fontFamily: JetBrains Mono
    fontSize: 10px
    fontWeight: '500'
    lineHeight: 14px
    letterSpacing: 0.08em
rounded:
  sm: 0.125rem
  DEFAULT: 0.25rem
  md: 0.375rem
  lg: 0.5rem
  xl: 0.75rem
  full: 9999px
spacing:
  gutter: 1rem
  gutter-sm: 0.75rem
  gutter-lg: 1.5rem
  margin: 1.5rem
  margin-sm: 1rem
  margin-lg: 2rem
  space-xs: 0.25rem
  space-sm: 0.5rem
  space-md: 1rem
  space-lg: 1.5rem
  space-xl: 2rem
---

## Brand & Style
The design system embodies the calculated clarity, rigor, and functional elegance of precision architectural blueprints and aerospace avionics schematics. Tailored for engineers, systems architects, and technical analysts navigating complex conversational workflows, the interface prioritizes spatial order, absolute legible hierarchy, and uncompromised structural definition over decorative flourish.

Drawing from modern technical brutalism filtered through high-fidelity drafting discipline, the style relies on fine-ruled architectural grid lines, high-contrast drafting ink tones, and monospaced telemetry data. Surfaces emulate calibrated drafting paper: crisp, radiant, and mathematically ordered. Interaction patterns evoke precision optical instruments—tactile snap, crisp boundary states, and clear focus reticles—instilling absolute trust, control, and intellectual focus.

## Colors
The palette is rooted in drafting paper neutrals and saturated technical inks:

- **Primary Blueprint Blue (`#2563eb`) & Cobalt Focus (`#1d4ed8`)**: Used for primary action affordances, key metric callouts, active terminal states, and precision focus rings. Communicates calculation, certainty, and active operations.
- **Secondary Slate/Navy (`#0f172a`)**: The deep technical ink applied to high-priority content, structural headers, and authoritative metrics. Supported by `#1e293b` and `#334155` for legible multiline technical prose.
- **Tertiary Cyan/Grid Blue (`#38bdf8`)**: Used for fine synthetic highlights, subtle telemetry indicators, active line cursors, and visual accents within complex diagrams.
- **Neutral & Drafting Paper Substrates**:
  - Base canvas: `#f8fafc` (Drafting Vellum Light) with subtle structural overlays of `#f1f5f9`.
  - Precision grid lines & architectural dividers: `#e2e8f0` (Level 1) and `#cbd5e1` (Level 2 structural frames).
  - Telemetry & muted metadata: `#64748b` (Steel Slate).
- **Functional Semantics**:
  - Success / Nominal: `#059669` (Emerald Drafting Green)
  - Warning / Delta: `#d97706` (Amber Precision Alert)
  - Critical / Fault: `#dc2626` (Redline Precision Error)

## Typography
The typographic hierarchy merges three disciplined typefaces:

1. **Space Grotesk** for structural headlines and major modal titles. Its geometric, slightly mechanical construction provides a technical, forward-looking architectural tone without degrading legibility.
2. **Geist** for body copy, chat dialogue, system output, and conversational payloads. Geist provides a clean, neutral, highly legible sans-serif texture that balances technical precision with high reading endurance.
3. **JetBrains Mono** for technical metadata, status indicators, system prompts, syntax-highlighted code blocks, timestamp annotations, coordinate markers, and tab labels. It reinforces the blueprint schematic atmosphere.

All label typography defaults to tabular figures (`tnum`) and uppercase styling where telemetry or axis identifiers are rendered.

## Layout & Spacing
The layout operates on a strict 8px spatial grid, reinforced visually by an optional 24px subtle coordinate drafting matrix across workspace containers.

- **Grid Architecture**: The core chat workspace uses a multi-pane split:
  - Fixed-width or collapsible side rails for sessions, tree structures, and parameter panels (280px to 360px).
  - Fluid central terminal/chat stream bounded by a maximum reading width of 840px for chat bubbles and infinite width for data tables/diff views.
  - Right-hand contextual blueprint canvas (50% split on screens > 1440px) for real-time schemas, code inspection, or visualization.
- **Rhythm & Alignments**: Structural dividers align to exact pixel coordinates using 1px solid borders. Gutters never float arbitrarily; components lock into horizontal lines and card edge markers that evoke CAD guidelines.
- **Breakpoints**:
  - Mobile (<768px): Single pane with bottom navigation sheet; structural margins reduce to `margin-sm` (1rem).
  - Tablet (768px - 1023px): Two-column layout with collapsable context panels.
  - Desktop (1024px+): Full multi-panel schematic layout with fixed 1px drafting lines separating workspaces.

## Elevation & Depth
In alignment with the blueprint drafting philosophy, this design system avoids diffuse drop shadows and blurred depth illusions. Depth and hierarchy are achieved through **crisp architectural borders**, **tonal layering**, and **hairline focus guides**:

- **Hairline Framing**: Panels and cards sit within 1px solid outlines in `#e2e8f0` (resting) and `#cbd5e1` (interactive or hover).
- **Surface Layering**:
  - Canvas / Foundation: `#f8fafc`
  - Active Work Surface / Message Cards: `#ffffff`
  - Inset Panels / Code Blocks / Secondary Telemetry: `#f1f5f9`
- **Zero Heavy Drop Shadows**: Elevated elements (such as tooltips, context menus, and active modal sheets) utilize a sharp, high-precision CAD offset:
  - `box-shadow: 0 1px 3px 0 rgba(15, 23, 42, 0.06), 0 1px 2px -1px rgba(15, 23, 42, 0.04);`
  - Floating menus add a 1px border of `#cbd5e1` to reinforce tactile edge boundaries.
- **Reticle Focus Rings**: Interactive states emit an outer 2px solid `#2563eb` outline offset by 1px white space, imitating drafting selection crosshairs.

## Shapes
Shapes emphasize engineering geometry. The roundedness level is set to `1` (Soft), yielding subtle 4px (`0.25rem`) standard corner radiuses that evoke milled industrial parts and architectural templates rather than playful consumer bubbles.

- **Standard Elements (Buttons, Inputs, Badges, Chat Insets)**: `rounded` (4px / 0.25rem).
- **Structural Containers & Modal Frames**: `rounded-lg` (8px / 0.5rem).
- **Precision Chips & Telemetry Tags**: `rounded-sm` (2px) or strictly squared (`0px`) with chamfered visual cues.
- **No Pill Shapes**: Rounded-full pill geometries are reserved exclusively for microscopic status pings and toggle thumbs to maintain an uncompromising technical presence.

## Components

### Buttons
- **Primary**: Solid blueprint blue (`#2563eb`) with white text, crisp 4px corners, 1px border of `#1d4ed8`. Hover transitions to `#1d4ed8`. Focus exhibits a 2px offset drafting ring.
- **Secondary / Ghost**: White or transparent background with 1px border in `#cbd5e1`, text in `#0f172a`. Hover state switches surface to `#f1f5f9` with border `#94a3b8`.
- **Engineering Action (Terminal style)**: `JetBrains Mono` label, uppercase, flanked by subtle bracket symbols (e.g., `[ RUN SYSTEM ]`).

### Chat Bubbles & Threads
- **System / Assistant Prompt**: White card (`#ffffff`), 1px solid `#e2e8f0`, corner radius 4px. Left-hand border features a 3px accent bar in `#2563eb` with a monospaced identifier (`SYS_OUTPUT // 0x4F`).
- **User Prompt**: Muted background (`#f1f5f9`), 1px solid `#cbd5e1`, aligned right or distinctively flagged with a navy header.
- **Code & Trace Modules**: Embedded directly into the bubble thread using `#0f172a` (dark terminal theme) or `#f8fafc` (light blueprint theme) bounded by 1px `#e2e8f0` with full line numbers and copy-reticle icons.

### Inputs & Chat Console
- Multi-line conversational prompt box anchored to the bottom dock: `#ffffff` surface, 1px solid `#cbd5e1`, 4px radius.
- Includes a top accessory toolbar displaying parameter chips (model temperature, execution context, token meter in `JetBrains Mono`).
- Focus state: Border shifts to `#2563eb` with an immediate 2px drafting halo.

### Chips & Telemetry Badges
- Background `#f1f5f9`, border 1px solid `#e2e8f0`, text `#334155` in `JetBrains Mono` (11px).
- Status variant: Includes a 6px geometric square dot indicator (green for nominal, blue for syncing, amber for warning).

### Checkboxes & Radio Buttons
- Crisp 14px boxes with 2px radius and 1px solid `#94a3b8` border.
- Checked state: `#2563eb` fill with a sharp, geometric checkmark icon or solid center square.

### Workspace Blueprint Cards & Inspectors
- Used for telemetry panels, diff views, and inspector sidebars.
- Features a header panel styled with a 1px bottom border in `#e2e8f0`, background in `#f8fafc`, uppercase monospaced section titles, and action icons.
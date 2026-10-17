# Agent instructions for `sagex-varta`

These instructions add crate-specific guidance to the repository-level `AGENTS.md`. Follow both files. The repository-level instructions remain in force; this file does not replace or weaken them.

## UI components

- When adding or changing a component, first check whether the needed behavior can be built by extending GPUI base primitives. Prefer that approach when it fits.
- If GPUI base primitives do not fit, create a custom component for this crate.
- Use a GPUI Kit component only when its appearance matches the required design pixel for pixel. If it does not, build or adapt a component locally.
- Do not use free functions to construct or return component trees. Implement components as GPUI `RenderOnce` types instead.
- Keep durable interactive state in the appropriate GPUI entity; pass state handles and callbacks into `RenderOnce` components as needed.
- Prefer reusable components and modular code. Break large render implementations into focused components rather than accumulating unrelated UI in one render tree.

## UI annotations

- Use the `//(!) <component name>` comment marker to annotate the requested UI components at their implementation in `src/`.
- Keep these annotations concise and place them beside the relevant component or behavior branch; do not rename components to match an annotation label.
- Preserve the marker spelling exactly as `//(!)` when adding or moving annotations.
- Annotated UI areas include avatars, icon buttons, filled and icon-label buttons, input fields, the new-conversation dropdown, conversation list rows and pinned state, chat composer, mode slider, incoming and outgoing chat messages, and message cards.

## Crate conventions

- Follow the GPUI Kit Coding Guides for framework, state ownership, identity, events, and API decisions. Follow the GPUI Kit Design Guides for UI structure, styling, interaction states, and interface copy.
- Preserve the existing module organization, theme conventions, and asset-generation flow in this crate.
- Keep changes scoped to this crate unless the requested work requires a repository-level change. Follow the repository-level low-memory and test rules for Cargo checks and tests.

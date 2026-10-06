# Product Requirements Document (PRD) & Project Brief: Blueprint Chat

**Document Revision:** 2.4.0  
**Project Code:** `DWG NO: 804-CHAT`  
**Status:** Approved for Implementation  
**Target Environment:** Web / Desktop Workstation (Electron / PWA / WebGL)  
**Design Reference:** Blueprint Precision Design System (`{{DATA:DESIGN_SYSTEM:DESIGN_SYSTEM_1}}`)

---

## 1. Executive Summary & Vision

### 1.1 Product Vision

**Blueprint Chat** is a high-precision, CAD-inspired collaboration workbench engineered for graphics programmers, hardware developers, algorithm specialists, and technical teams. Unlike consumer messaging clients (Slack, Discord, Teams) that treat code, math, and telemetry as passive text attachments, Blueprint Chat models technical collaboration as an interactive drafting and schematic canvas.

### 1.2 Core Value Proposition

- **CAD & Blueprint Drafting Aesthetic:** Crisp hairline dividers, coordinate grids, monospace metrics, and architectural title blocks that evoke engineering precision and reduce visual friction for technical users.
- **First-Class Technical Artifacts:** Native support for GLSL/WGSL shader inspection, vector topology matrices, memory allocation tracking, and schematic node discussions.
- **Context-Driven Multitasking:** Modular, resizable panel architecture featuring dual sidebars, horizontal inspection shelves, and persistent property inspectors.

---

## 2. Target Audience & Personas

1. **Shader & Graphics Engineers (Primary):**
    - _Needs:_ Inline GLSL shader previewing, channel value matrix debugging (RGB clamped/linear), fast compilation log sharing.
    - _Pain Point:_ Formatting complex math, pipeline DAGs, and fragment kernels in standard chat tools leads to degraded readability.
2. **Systems & Algorithm Architects:**
    - _Needs:_ Microsecond-level telemetry, packet tracking, pipeline block diagrams, and strict revision locks.
    - _Pain Point:_ Lack of spatial context when referencing multi-stage pipelines across message threads.
3. **Hardware / FPGA / Embedded Developers:**
    - _Needs:_ Register maps, coordinate referencing (`LOC: S1-01`), hardware synthesis passes, and low-latency protocol verification.

---

## 3. Product Architecture & Layout Specifications

Based on finalized UI milestones (`{{DATA:SCREEN:SCREEN_19}}`, `{{DATA:SCREEN:SCREEN_15}}`, `{{DATA:SCREEN:SCREEN_13}}`), the application is structured around a flexible 4-tier layout:

### 3.1 Primary Navigation Dock (64px fixed)

- **Workspace Selector:** Branded isometric chat node emblem (`{{DATA:IMAGE:IMAGE_21}}`) with active indicator.
- **Core Modes:**
    - `CHATS` (Active node streams)
    - `THRDS` (Sub-schematic threads)
    - `FILES` (DWG attachments & shader notebooks)
    - `TOOLS` (Drafting kernel utilities, GLSL runners)
- **Dock Utilities:** Light/Dark theme toggle, viewport projection modes, configuration settings (`CFG`), port receptor status (`PORT: 8080 // WS`).

### 3.2 Channels & Active Nodes Sidebar (280px resizable)

- **Technical Search:** Omnibox supporting CAD node indexing with keyboard shortcuts (`Cmd+K`).
- **Pinned Nodes:** Dedicated tier with stage indicators (`STAGE 1`, timestamped sync status, pin toggle).
- **Schematic Streams:** Real-time channel list with unread packet badges, status dots (green = online, red = error), and context action triggers (`...`).
- **Session Telemetry Footer:** Active operator status, latency stats, and calibration status (`CAL: OK`).

### 3.3 Central Collaboration Canvas (Fluid flex-grow)

- **Drafting Ribbon Header:** Architectural title block displaying drawing number (`DWG NO: 804-CHAT`), scale (`1:1`), revision (`REV: 2.4`), viewport coordinate mapping (`X:142 Y:892`), and kernel status (`ONLINE DRAFTING KERNEL`).
- **Interactive Message Stream:**
    - **Coordinate Timestamps:** ISO-timestamped markers (`T_ZERO: 14:12 UTC`, `LOC: S1-01`).
    - **Schematic Diagram Cards:** Inline layer-blend pipeline visuals, node topology trees, and channel value matrices.
    - **Code Kernel Blocks:** Syntax-highlighted GLSL/C++ blocks with line numbering, SHA-256 hash validation, one-click copy, and runner bindings.
    - **Reaction & Operations Rail:** Message context menu triggers, emoji telemetry reactions, quote-reply, and thread branching.
- **Precision Composer Tray:**
    - Multi-mode transmission switcher (`MESSAGE`, `MARKDOWN`, `CODE PATCH`, `CLI COMMAND`).
    - Input area supporting formula autocompletion, technical parameters, and attachment linking.
    - Send button with shortcut trigger (`Cmd+Enter transmit`).

### 3.4 Technical Inspector & Property Sheet (300px collapsible / dockable)

- **Element Properties:** Creation timestamp, member roster, message packet counts, log retention locks (e.g., 90 days locked), frame rate metrics.
- **Tag Registry:** Semantic taxonomy pills (`rendering`, `shaders`, `cv`, `imgproc`).
- **Operators Roster:** Real-time presence list with role badges (`OWNER`, `CORE-ALGO`, `AUTHOR`, `ONLINE`, `AWAY`).
- **Sub-Schematics (Threads):** Thread trees with reply counters and timestamp diffs.
- **DWG Attachments:** Technical file assets with mime badges, file weight, and quick download/preview utilities.
- **Live Telemetry Card:** End-to-end ping (`28 ms`), packet rate (`142 pkt/s`), and heap allocation gauges.

---

## 4. Key Functional Requirements

| Ref ID    | Feature Category           | Specification / Acceptance Criteria                                                                                                       | Priority    |
| :-------- | :------------------------- | :---------------------------------------------------------------------------------------------------------------------------------------- | :---------- |
| **FR-01** | **Drafting Canvas Grid**   | Provide subtle dot/millimeter coordinate background with dynamic grid ticks and pan-zoom awareness.                                       | Must-Have   |
| **FR-02** | **Technical Context Menu** | Context menu on messages and channels offering: _Reply in Thread_, _Quote Message_, _Inspect Node_, _Copy Shader Ref_, _Export Pipeline_. | Must-Have   |
| **FR-03** | **Interactive Schematics** | Render vector pipeline cards directly within chat bubbles with dynamic input/output channel readouts.                                     | Must-Have   |
| **FR-04** | **Dual Theme Support**     | Support high-contrast technical Light Mode (`#ffffff`, `#eff4ff`) and deep Blueprint Dark Mode (`#0d1322`, `#12131a`).                    | Must-Have   |
| **FR-05** | **Modular Docking**        | Allow inspector to dock as a right-hand sidebar or a bottom horizontal telemetry tray (`{{DATA:SCREEN:SCREEN_13}}`).                      | Should-Have |
| **FR-06** | **GLSL Shader Execution**  | Embedded WebGL/Canvas renderer supporting live preview and parameter tweaking for shader attachments.                                     | Should-Have |
| **FR-07** | **PWA & Offline Drafting** | Complete Web App Manifest integration (`{{DATA:DOCUMENT:DOCUMENT_9}}`) with local SQLite/IndexedDB caching.                               | Must-Have   |

---

## 5. Non-Functional Requirements & Design Tokens

### 5.1 Performance & Latency

- **Frame Rate:** 60 FPS scrolling throughout dense technical message logs containing canvas/SVG diagrams.
- **Message Delivery:** < 50ms transport latency over secure WebSockets (`PORT: 8080 // WS`).
- **Memory Footprint:** Initial memory footprint < 120MB on desktop Electron clients.

### 5.2 Design Tokens & Styling (Blueprint Precision)

- **Primary Accent:** Cobalt Blueprint (`#2563eb`), Neon Laser Cyan (`#38bdf8`), Deep Navy (`#0f172a`).
- **Typography:** `Space Grotesk` (UI Headers & Title Blocks), `Geist Mono` / `Courier New` (Code & Coordinates).
- **Border Radii:** Clean `rounded-sm` (4px) and circular user nodes (`rounded-full`).
- **Dividers:** 1px hairline borders (`border-blue-200/40` in light mode, `border-blue-900/40` in dark mode).

---

## 6. Implementation Milestones & Roadmap

<!--
- **Phase 1 (Alpha): Core Layout & Realtime Node Streaming**
  Implement the 4-column layout, WebSocket chat client, message composer with markdown/code tabs, and user status indicators.
- **Phase 2 (Beta): Schematic Rendering Engine & Context Menus**
  Add interactive pipeline diagram rendering, context action popovers, and thread inspector trays.
- **Phase 3 (RC): Shader Previewer & Workspace Customization**
  Integrate live WebGL canvas preview, customizable dockable panels, and full PWA offline support.
- **Phase 4 (v1.0): Enterprise Security & Retention**
  End-to-end cryptographic packet signing, telemetry logs, and CAD file format import/export.-->

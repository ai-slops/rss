# Live Production Design System

## 0. Research Log

- Embedded refs: shortlisted Sentry, IBM Carbon, and Runway → picked operational `taste-skill` + Sentry because the product needs a dark, data-dense technical console with tactile controls, while replacing Sentry's brand purple with broadcast semantics.
- Lazyweb: 1 targeted desktop query, 4 screens viewed (Grass Valley Masterpiece X, Meld Studio, NVIDIA Broadcast, Restream Studio) → took Meld's fixed scene/canvas/mixer subdivision, Restream's rundown and setup rail, and Grass Valley's persistent broadcast status. Marketing-page composition was rejected.
- Imagen drafts: three-direction session research board → picked direction A, Broadcast Cockpit, as the reference-fidelity contract. It has explicit cyan Preview, red Program, amber warnings, rectangular panels, and a persistent lower operations band.
- Interaction refs: read beui.dev `button`, `tabs`, and `drawer` source → took interruptible state feedback, roving tab keyboard behavior, focus-returning drawers, and reduced-motion opacity fallbacks without copying visual styling.

## 1. Atmosphere & Identity

A precise broadcast cockpit: dense, calm, and unambiguous under time pressure. The interface should feel closer to calibrated production hardware than a generic admin dashboard. Its signature is the **dual bus**: Preview is always edged in cyan, Program is always edged in red, and scope tags make Draft, Live/shared, and This browser impossible to confuse. `DESIGN_VARIANCE: 3`, `MOTION_INTENSITY: 3`, `VISUAL_DENSITY: 9`.

Primary persona: a laptop operator who must edit while live without affecting Program. Secondary personas: a producer checking status in another browser, and an operator working with temporary motor or vision impairment who needs numeric and keyboard alternatives. Stress contexts include a dim room, a noisy venue, intermittent control connectivity, a lagging preview, 200% zoom, and one-handed trackpad use.

## 2. Color

The product has one dark theme. Color is semantic, never decorative.

| Role | Token | Value | Usage |
|---|---|---|---|
| Canvas | `--color-canvas` | `#090d10` | Application background |
| Surface 1 | `--color-surface-1` | `#10161b` | Main panels |
| Surface 2 | `--color-surface-2` | `#161e24` | Toolbars, selected regions |
| Surface 3 | `--color-surface-3` | `#1d2830` | Elevated controls and drawers |
| Surface sunken | `--color-surface-sunken` | `#070a0d` | Media wells and meters |
| Border subtle | `--color-border-subtle` | `#25323b` | Panel division |
| Border strong | `--color-border-strong` | `#3a4b57` | Focused structural edge |
| Text primary | `--color-text-primary` | `#f4f7f8` | Primary content |
| Text secondary | `--color-text-secondary` | `#a9b6be` | Metadata and explanations |
| Text muted | `--color-text-muted` | `#71818c` | Disabled and tertiary text |
| Preview | `--color-preview` | `#20b9d6` | Preview bus and Draft selection |
| Preview strong | `--color-preview-strong` | `#5bd7ee` | Preview focus and values |
| Program | `--color-program` | `#ef4056` | Program/on-air and destructive confirmation |
| Program strong | `--color-program-strong` | `#ff7182` | Program focus and critical values |
| Healthy | `--color-healthy` | `#71d68b` | Connected, ready, signal healthy |
| Warning | `--color-warning` | `#efb84f` | Pending, stale, risky public action |
| Info | `--color-info` | `#7ba6ff` | Browser-local and informational state |
| Meter low | `--color-meter-low` | `#63cb76` | Normal audio level |
| Meter high | `--color-meter-high` | `#e6c84f` | Near clipping |
| Meter clip | `--color-meter-clip` | `#f1535e` | Clipping |
| Scrim | `--color-scrim` | `rgb(1 4 6 / 72%)` | Modal and drawer backdrop |

Rules:
- Red never means generic selection. It means applied Program, on-air, destructive action, or recording.
- Cyan never means live. It means Preview, selected Draft, or safe preparation.
- Green means observed health, not a command acknowledgment.
- Every state includes text or an icon in addition to color.

## 3. Typography

Self-host when font assets become available. Until then use installed/local fallbacks without a network dependency.

| Level | Token | Size | Weight | Line height | Usage |
|---|---|---:|---:|---:|---|
| Workspace title | `--type-title` | `1.125rem` | 700 | 1.2 | Project and drawer titles |
| Panel title | `--type-heading` | `0.8125rem` | 700 | 1.25 | Panel headings |
| Body | `--type-body` | `0.8125rem` | 450 | 1.4 | Default UI copy |
| Small | `--type-small` | `0.75rem` | 450 | 1.35 | Metadata and helper text |
| Label | `--type-label` | `0.6875rem` | 650 | 1.2 | Control labels and scope tags |
| Telemetry | `--type-telemetry` | `0.75rem` | 550 | 1.2 | Timecode, bitrate, revisions |
| Timecode | `--type-timecode` | `1rem` | 650 | 1 | Program clock |

- Display/heading stack: `"Barlow Condensed", "Arial Narrow", sans-serif`.
- UI stack: `"IBM Plex Sans", "Noto Sans", system-ui, sans-serif`.
- Telemetry stack: `"IBM Plex Mono", "Cascadia Mono", monospace` with `font-variant-numeric: tabular-nums`.
- Body text is never below 12px. Tiny telemetry is not used for instructions or actions.

## 4. Spacing & Layout

All intent spacing derives from a 4px base.

| Token | Value | Usage |
|---|---:|---|
| `--space-1` | `0.25rem` | Tight icon/label gap |
| `--space-2` | `0.5rem` | Compact row gap |
| `--space-3` | `0.75rem` | Control padding |
| `--space-4` | `1rem` | Panel padding |
| `--space-5` | `1.25rem` | Comfortable block gap |
| `--space-6` | `1.5rem` | Drawer section gap |
| `--space-8` | `2rem` | Empty-state spacing |

Other geometry tokens: `--radius-control: 0.25rem`, `--radius-panel: 0.375rem`, `--control-min: 2.25rem`, `--control-comfortable: 2.75rem`, `--header-size: 3rem`, `--operations-size: 15.5rem`, `--rail-size: 3.25rem`.

The desktop workspace is a `scroll-body-shell`: header, notice stack, production grid and operations band occupy explicit grid rows; an empty notice stack must not move production into an auto-sized row. Each list/panel owns its named scroll. The production grid is scenes/layers | Preview and Program | inspector. Below 1100px the Inspector remains keyboard-reachable in a full-width row below the monitors. Below 760px monitors and Inspector stack vertically; full mobile canvas drag editing is not promised, but numeric alternatives remain available. Short desktop viewports permit document scrolling instead of clipping fixed regions, including at 200% zoom. At 375px primary content reflows with no page-level horizontal scroll. Operator-assigned names wrap naturally rather than becoming an empty ellipsis.

## 5. Components

### Button
- **Structure**: native `button`, icon optional, one-line label.
- **Variants**: neutral, preview, program, danger, ghost, icon.
- **States**: default, hover, pressed inset, focus-visible, disabled, pending, success, error.
- **Accessibility**: minimum 36px desktop target, 44px in focused/mobile modes; icon buttons have accessible names.
- **Motion**: 80ms press feedback using transform only; pending label swaps without changing command meaning.

### Status Chip
- **Structure**: state icon, short label, optional telemetry.
- **Variants**: healthy, warning, error, info, stale, unknown.
- **Accessibility**: state text always present; changing critical state uses a restrained live region.
- **Layout**: cluster, never decorative.

### Scope Tag
- **Structure**: icon plus one of `DRAFT`, `LIVE / SHARED`, `THIS BROWSER`.
- **Variants**: preview cyan, warning/live amber, browser info blue.
- **Rule**: appears at section level instead of repeated on every control when scope is shared.

### Panel
- **Structure**: heading row, optional toolbar, scroll body, optional status footer.
- **States**: normal, selected, stale, disabled, loading, empty, error.
- **Layout**: stack inside a bounded region; the body is the only scroll owner.

### Media Monitor
- **Structure**: labelled header, 16:9 media frame, state overlay, revision/timestamp footer.
- **Variants**: Preview, Program, compact Program.
- **States**: live media, connecting, stale frame, disconnected, display confirmation unavailable.
- **Accessibility**: monitor labels are text; no autoplay audio; monitoring requires explicit activation.

### Scene Row and Layer Row
- **Structure**: thumbnail/type icon, name, state metadata, scoped row actions.
- **States**: selected, matches Program, on-air dependency, hidden, locked, disconnected source, stale revision.
- **Keyboard**: arrow navigation, Enter selects/cues only, Delete requires eligible state and confirmation. Double click never Takes.

### Segmented Tabs
- **Structure**: `tablist`, roving-focus tabs, labelled `tabpanel`.
- **Behavior**: Arrow keys, Home, End; active indicator moves instantly under reduced motion.
- **Use**: Layers/Scenes modes, setup page categories, inspector sections. It is never used for destructive commands.

### Field and Numeric Stepper
- **Structure**: visible label, input/select, unit, helper/error text, step buttons where useful.
- **States**: default, focus, invalid, disabled, pending, conflict.
- **Accessibility**: numeric editing is a non-drag alternative for every canvas transform and sync value.

### Mixer Strip
- **Structure**: track label, pre/post meter, vertical range, numeric dB value, mute, route state, listen.
- **States**: routed, unrouted, muted, clipping, disconnected, selected listen.
- **Rule**: Listen is browser-local and visually separated from Live mute.

### Output Row
- **Structure**: name/type, desired state, observed state, last media activity, bitrate/error, explicit action.
- **States**: stopped, starting, sending, reconnecting, stopping, failed, platform-live, platform-unknown.
- **Safety**: public-risk destinations require a confirmation dialog that names the destination and says media may become public.

### Source Recovery Controls
- **Structure**: source-scoped policy checkboxes, textual controller/receiver facts, retry budget, explicit Apply, Retry, Disable, and Revoke actions.
- **States**: status unavailable, disabled, maintenance, warming, healthy, stalled, source disabled, controller offline, conflict, and pending mutation.
- **Safety**: no remote Start. Manual OBS Start arms recovery; intentional Stop is never reversed. Disable cancels future actions and Revoke invalidates the scoped controller.
- **Truthfulness**: cached healthy state is hidden when polling fails; unknown counters remain `Unknown`; process restart is explicitly unsupported.
- **Security**: enrollment tokens remain CLI-only and are never rendered or persisted by the browser.

### Drawer and Dialog
- **Structure**: labelled modal surface, close action, section body, action footer.
- **Behavior**: focus enters, remains trapped, Escape closes when safe, focus returns to opener. Destructive/public operations use dialogs; setup uses drawers/pages.
- **Motion**: transform/opacity only; opacity-only fallback for reduced motion.

### Toast and Inline Notice
- **Toast**: transient, non-critical acknowledgment only.
- **Inline notice**: errors, stale state, conflict recovery, and blocked dependencies remain next to the affected control.

## 6. Motion & Interaction

| Token | Value | Use |
|---|---|---|
| `--motion-press` | `80ms` | Physical button press |
| `--motion-fast` | `120ms` | Hover/focus/color feedback |
| `--motion-panel` | `180ms` | Drawer and panel state |
| `--ease-out` | `cubic-bezier(0.16, 1, 0.3, 1)` | UI state arrival |

- Motion communicates command state, pane relationship, or focus only. Meters are real observations, not decorative animation.
- Only `transform`, `opacity`, and color/filter transitions animate. No layout-property animation.
- `prefers-reduced-motion: reduce` makes panel changes instant and retains status feedback through text/icon changes.
- Global shortcuts: `Ctrl/Cmd+Z` and `Ctrl/Cmd+Shift+Z` for draft undo/redo, arrow-key nudging on selected layers, Escape cancels an active gesture or closes the top safe overlay. Shortcuts do not fire from inputs, textareas, selects, contenteditable elements, IME composition, or dialogs.
- Take has no single-key shortcut. Output start/stop has no default global shortcut.

## 7. Depth & Surface

Strategy: mixed tonal shift plus sparse borders and tactile inset controls. Panels use tonal differences and a single subtle edge. Drawers/dialogs may use one elevated shadow. Media wells and fader tracks are sunken. No outer glow, glassmorphism, or decorative noise.

| Token | Value | Usage |
|---|---|---|
| `--shadow-inset` | `inset 0 1px 2px rgb(0 0 0 / 35%)` | Pressed controls and wells |
| `--shadow-control` | `inset 0 1px 0 rgb(255 255 255 / 5%), 0 1px 1px rgb(0 0 0 / 30%)` | Resting controls |
| `--shadow-overlay` | `0 1.25rem 3rem rgb(0 0 0 / 45%)` | Dialogs and drawers only |

## 8. Accessibility Constraints & Accepted Debt

### Constraints

- WCAG 2.2 AA target: 4.5:1 body contrast, 3:1 large text and component boundaries, visible focus on every interactive element.
- Complete keyboard reachability and logical focus order. Pointer drag always has numeric/button alternatives.
- Do not rely on color, motion, spatial position, audio, or a frozen preview alone to communicate state.
- Support 200% zoom and browser text scaling. At narrow widths the operator chooses a pane rather than receiving a squeezed unreadable cockpit.
- Announce control disconnect, Program application, output failures, and recording failures without flooding live regions with meters or timecode.
- Authentication errors explain whether the session expired, access is denied, or the server is unreachable. Credentials never appear in URLs or persisted browser drafts.

### Accepted Debt

| Item | Location | Why accepted | Owner / Exit |
|---|---|---|---|
| Full mobile drag editing | Production canvas | First release only promises small-laptop focused panes; numeric editing remains available | UI owner when mobile editing becomes a product requirement |
| Reliable browser frame-to-Program timestamp correlation | Media Monitor | Browser metadata support and backend signaling require Stage A validation | Coordinator and UI after preview protocol evidence |
| Self-hosted Barlow Condensed and IBM Plex assets | Global typography | Repository begins without font assets; local fallbacks avoid third-party requests | UI owner before branded release |

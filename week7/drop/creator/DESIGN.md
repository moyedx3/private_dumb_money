# Creator Drop Design System

## 1. Atmosphere & Identity

Creator Drop feels like a restrained security workbench: compact, legible, and calm under operational pressure. The signature is verified-progress clarity, where each panel keeps one provisioning concern visible and the Status rail makes the encrypt, attest, and provision sequence easy to audit without exposing secret material.

## 2. Color

### Palette

| Role | Token | Light | Dark | Usage |
|------|-------|-------|------|-------|
| Surface/page | `--surface-page` | `#f7f7f2` | `#17201a` | App background and page-level canvas |
| Surface/panel | `--surface-panel` | `#ffffff` | `#17201a` | Panel bodies and grouped tool surfaces |
| Surface/input | `--surface-input` | `#ffffff` | `#17201a` | Form field backgrounds |
| Text/primary | `--text-primary` | `#17201a` | `#f7f7f2` | Body copy, headings, form input text |
| Text/secondary | `--text-secondary` | `#596156` | `#d9ddd2` | Notes, step labels, result labels, eyebrow text |
| Text/field-label | `--text-field-label` | `#4c564d` | `#d9ddd2` | Form field labels |
| Border/panel | `--border-panel` | `#d9ddd2` | `#596156` | Panel outlines and major separations |
| Border/control | `--border-control` | `#cbd1c7` | `#596156` | Inputs, textareas, idle step dots |
| Accent/primary | `--accent-primary` | `#0f6b50` | `#99c7b8` | Primary button, success text, done state, focus border |
| Accent/focus | `--accent-focus` | `#99c7b8` | `#99c7b8` | Keyboard focus outline |
| Status/success | `--status-success` | `#0f6b50` | `#99c7b8` | Success messages and completed steps |
| Status/warning | `--status-warning` | `#d9972b` | `#d9972b` | Running or pending operational state |
| Status/error | `--status-error` | `#9a261b` | `#d9972b` | Error text and failed step markers |

### Rules

- Use only the roles above for creator UI work. New hex values must first be added here with a semantic role.
- The green accent is reserved for trusted action, verified success, and focus confirmation. It is not decorative.
- Warning and error colors must appear only in Status, validation, or failure messages.
- The current app is light-first. Dark values exist only as future equivalents; do not add dark mode until the whole surface is implemented.

## 3. Typography

### Scale

| Level | Size | Weight | Line Height | Tracking | Usage |
|-------|------|--------|-------------|----------|-------|
| Display | `clamp(2rem, 5vw, 3.6rem)` | 700 | 1 | 0 | Main page heading |
| H2 | `1.05rem` | 700 | 1.25 | 0 | Panel heading |
| Body | `1rem` | 400 | 1.55 | 0 | Default copy, field values, step labels |
| Body/sm | `0.9rem` | 400 | 1.5 | 0 | Monospace result values |
| Label | `0.85rem` | 700 | 1.35 | 0 | Form field labels |
| Caption | `0.8rem` | 700 | 1.3 | 0 | Eyebrow label |
| Result label | `0.78rem` | 800 | 1.3 | 0 | Uppercase result metadata |

### Font Stack

- Primary: `Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`
- Mono: `ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace`
- Serif: none

### Rules

- Typography stays utilitarian and compact. Do not introduce marketing hero scale beyond the existing Display level.
- Letter spacing remains `0`, including headings, except browser default uppercase rendering for labels.
- Use the mono stack only for hashes, byte counts, identifiers, and other machine-verifiable output.

## 4. Spacing & Layout

### Base Unit

Spacing follows a 4px base with the current CSS values mapped into stable tokens.

| Token | Value | Usage |
|-------|-------|-------|
| `--space-1` | `4px` | Result label-to-value gap |
| `--space-2` | `8px` | Tight label stacks, compact local gaps |
| `--space-3` | `12px` | Pair grid gap, step marker size |
| `--space-4` | `16px` | Main grid gap, responsive shell side margin |
| `--space-5` | `20px` | Comfortable inline control padding equivalent |
| `--space-6` | `24px` | Toolbar gap and section rhythm |
| `--space-8` | `32px` | Shell vertical padding |
| `--space-panel` | `18px` | Panel internal padding and primary button horizontal padding |
| `--space-field-y` | `10px` | Form field vertical padding |
| `--space-field-x` | `11px` | Form field horizontal padding |

### Grid

- Max content width: `1120px`.
- Shell width: `min(1120px, calc(100vw - 32px))`.
- Main grid: two equal columns with `16px` gap; `.wide` spans both columns.
- Pair grid: two equal columns with `12px` gap.
- Mobile breakpoint: `760px`; toolbar, main grid, and pair grid collapse to one column.

### Rules

- Keep the first viewport a usable creator tool, not a landing page.
- Preserve the compact dashboard density. Add space only when it improves scanning or prevents input crowding.
- Avoid nested cards. Panels are top-level work zones; repeated results inside a panel should remain unframed.

## 5. Components

### Primary button

- **Structure**: single `button.primary` in the toolbar.
- **Variants**: default, hover, active, focus-visible, disabled, loading-disabled.
- **Spacing**: min height `44px`, horizontal padding `--space-panel`, border radius `6px`.
- **States**: default uses `--accent-primary`; disabled keeps layout stable with `opacity: 0.45` and `not-allowed`; focus uses `--accent-focus`; loading must reuse disabled treatment while a step is running.
- **Accessibility**: must be a real button with an accessible command label. Disabled state must prevent duplicate submit.
- **Motion**: optional micro `opacity` or `transform` feedback only, `100-150ms ease-out`.

### Panel

- **Structure**: `section` or `div` with `panel` class containing one `h2` and related controls or Status content.
- **Variants**: standard two-column panel, `wide` full-row panel.
- **Spacing**: grid gap `--space-4`, padding `--space-panel`, border radius `8px`.
- **States**: panels do not change color by status; status is expressed by messages and step markers inside the panel.
- **Accessibility**: headings must describe the panel purpose, such as Indexer Trust, Drop Metadata, Content, and Status.
- **Motion**: no entrance motion by default.

### Form field

- **Structure**: label text wraps a single `input` or `textarea`.
- **Variants**: text, numeric, decimal, file, textarea, disabled textarea.
- **Spacing**: label stack gap `7px`; field padding `--space-field-y` and `--space-field-x`; border radius `6px`.
- **States**: default border `--border-control`; focus outline `2px solid --accent-focus` with `--accent-primary` border; disabled fields must remain readable and visibly inactive.
- **Accessibility**: every control uses a visible label. Placeholder text supplements labels but never replaces them.
- **Motion**: focus transitions may use `border-color` and outline changes without layout shift.

### Status

- **Structure**: a Status panel with `.steps`, each `.step` containing a circular marker and label.
- **Variants**: idle, running, done, error.
- **Spacing**: steps gap `10px`; marker `12px`; row min height `32px`; row gap `10px`.
- **States**: idle marker uses `--border-control`; running marker uses `--status-warning`; done marker uses `--status-success`; error marker uses `--status-error`.
- **Accessibility**: status messages must name the failing stage and must not render `k_drop`, `K_drop`, `creator_ufvk`, or UFVK values.
- **Motion**: status changes can fade or scale markers with micro timing; do not animate row height.

### Result

- **Structure**: definition list for public, non-secret output.
- **Variants**: hash value, byte count.
- **Spacing**: list gap `10px`; item gap `--space-1`.
- **States**: result appears only after success.
- **Accessibility**: labels stay text-visible and values use overflow wrapping for long hashes.
- **Motion**: optional opacity-in using standard timing.

## 6. Motion & Interaction

### Timing

| Type | Duration | Easing | Usage |
|------|----------|--------|-------|
| Micro | `100-150ms` | `ease-out` | Button press, hover, focus emphasis |
| Standard | `200-300ms` | `ease-in-out` | Result or message reveal |
| Emphasis | none by default | none | Not used in the creator dashboard |

### Rules

- Animate only `transform`, `opacity`, or color-related paint changes. Never animate layout properties.
- Every interactive element needs visible hover, active, focus, disabled, and loading behavior when implemented.
- Respect `prefers-reduced-motion` by disabling non-essential transitions.
- Motion must clarify workflow state; it must not distract from attestation or provisioning evidence.

## 7. Depth & Surface

### Strategy

Creator Drop uses a borders-only surface strategy with restrained tonal contrast. The page background is off-white, panels and fields are white, and depth comes from subtle borders rather than shadows.

| Type | Value | Usage |
|------|-------|-------|
| Panel border | `1px solid var(--border-panel)` | Panel outlines and large grouped surfaces |
| Control border | `1px solid var(--border-control)` | Form fields and idle status markers |
| Focus outline | `2px solid var(--accent-focus)` | Keyboard and input focus confirmation |

### Rules

- Do not add shadows, glass effects, gradient orbs, decorative glows, or nested card surfaces.
- The surface hierarchy is page, panel, form field/result. New surfaces must fit one of those levels.
- Use borders and small tonal changes to separate work areas; keep operational information dense and scannable.

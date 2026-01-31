# Appearance Light Theme (macOS Light) Design

Date: 2026-01-31
Status: Approved

## Goal
Provide a macOS Light look-and-feel while keeping the existing VS Code Dark Modern style intact. The light theme should be instantly recognizable as Apple-style light UI, with high readability and minimal behavioral risk.

## Non-Goals
- No new layout or component structure changes.
- No transparent/vibrancy effects.
- No redesign of existing UI flows.

## Approach
Use CSS variable swapping only. All components continue to use existing utility classes (e.g. `bg-vscode-*`, `text-vscode-*`, `border-vscode`). The variables defined in `:root` remain the dark defaults; a new `html[data-theme='light']` block overrides variables with macOS Light palette values. This makes the theme change global and centralized.

## Theme Application
- On app startup, load config and call `initTheme(config.theme)` to apply.
- On settings save, call `setTheme(config.theme)` for immediate update.
- For `system`, resolve to `light` or `dark` via `matchMedia` and apply that to `data-theme`.

DOM state:
- `document.documentElement.dataset.theme = 'light' | 'dark'`
- `document.documentElement.classList.toggle('dark', resolved === 'dark')`

## Palette (macOS Light)
- Backgrounds: editor `#f5f5f7`, sidebar `#f2f2f7`, panel `#ffffff`, input `#ffffff`
- Borders: `#d1d1d6` (base), `#e5e5ea` (light)
- Text: `#1c1c1e` (primary), `#3a3a3c` (dim), `#8e8e93` (muted)
- Accent: `#0a84ff` (hover `#409cff`)
- Status: success `#34c759`, warning `#ff9f0a`, error `#ff3b30`, info `#007aff`
- Selection/list: selection `#d7ebff`, hover `#f0f0f2`, active `#e5e5ea`

## Testing
Manual smoke:
1) Launch app in `system` and verify it follows OS.
2) Set `light` and verify immediate and persisted effect after restart.
3) Set `dark` and verify immediate and persisted effect.
4) Set back to `system` and confirm OS theme changes are reflected.
5) Spot-check key surfaces (Settings, Brainstorm, dialogs, lists, inputs) for contrast and readability.

## Risks
- Any hardcoded colors outside variables may look off in light mode.
- If a platform reports `prefers-color-scheme` inconsistently, `system` might appear out of sync. Use `data-theme` as the single source of truth.

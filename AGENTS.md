# Repository Guidelines

## Language

- Code, comments, error messages, logs, UI copy, and README content in English.

## UI/UX

- Visual baseline: Memos-style neutral grayscale with subtle overlay outlines, restrained radii, no decorative shadows, and visible focus indicators. Preserve semantic priority/status cues.
- Use semantic design tokens only — no hardcoded colors, gradients, glass effects, heavy cards, excessive pills, or module-specific color themes. Feedback colors stay semantic (green success, amber warning, red error/destructive) and always pair color with text or an icon.
- Keep UI clean: drop redundant copy, labels, separators, borders, and containers; every element must serve content, action, state, or orientation. Build hierarchy with spacing, alignment, typography, scale, and weight before adding dividers or surfaces.
- Typography: Work Sans for app UI, monospace for code/timestamps/tags; reader fonts may be customized. Spacing base 4px, control radii 8px, surface/overlay radii 12px.
- List pages share the Memos page header and 920px container. Preserve Library split views, reader typography, compact Todo density, and Memo card borders.
- Reveal secondary list actions on hover, keyboard focus, or open overlay — and always keep them visible on mobile/touch. Reserve ellipsis icons for action menus.
- Responsive: full navigation + context rail at `>= 1200px`; icon rail + drawer at `768–1199px`; single column, top bar, bottom nav below `768px`. Mobile targets at least 44px, no horizontal scrolling, never require hover for primary actions.
- Prefer inline editing; confirm destructive actions; optimistic updates must roll back with clear errors. Preserve focus after edits, dialogs, and list changes. Support `Ctrl/Cmd + Enter`, `Ctrl/Cmd + K`, and `Esc`.
- Cover loading, empty, submitting, success, and error states — compact loading/empty on list pages, inline Field.Error for forms, StatusMessage for page-level errors, live regions for async results. Accessibility: visible `:focus-visible`, accessible names for icon buttons, WCAG 2.2 AA contrast, non-color state indicators, and purposeful motion within 100–220ms that respects `prefers-reduced-motion`.

## Code Style

- Avoid redundant comments; comment only non-obvious intent or constraints, concisely.

## Tests

- Test core behavior and important risk boundaries; skip low-value, repetitive, or implementation-detail tests.

## Git

- Keep commit messages concise.

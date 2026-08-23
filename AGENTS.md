# Repository Guidelines

## Language

- Write code, error messages, log messages, and UI copy in English.
- Write comments in English.
- Write README content in English.

## UI/UX

- Follow a calm editorial utility style: warm neutrals, dark text, one moss-green accent, restrained radii, and minimal shadows.
- Use semantic design tokens. Avoid hardcoded colors, gradients, glass effects, heavy cards, excessive pills, and module-specific color themes.
- Keep UI clean and purposeful. Remove redundant copy, repeated labels, decorative separators, borders, and containers; every visible element must support content, action, state, or orientation.
- Create hierarchy and grouping with spacing, alignment, typography, scale, weight, and restrained semantic color. Add dividers, borders, or extra surfaces only when these cues are insufficient.
- Use system UI fonts, monospace for code/timestamps/tags, a 4px spacing base, 8px control radii, and 12px surface or overlay radii.
- Responsive layout: full navigation and context rail at `>= 1200px`; icon rail and drawer at `768–1199px`; single column, top bar, and bottom navigation below `768px`.
- Keep mobile targets at least 44px, prevent horizontal scrolling, and never require hover for primary actions.
- Prefer inline editing. Confirm destructive actions; optimistic updates must roll back and show clear errors on failure.
- Preserve focus after edits, dialogs, and list changes. Support `Ctrl/Cmd + Enter`, `Ctrl/Cmd + K`, and `Esc`.
- Provide clear loading, empty, submitting, success, and error states. Announce errors and async results through appropriate live regions.
- Use visible `:focus-visible` states, accessible names for icon buttons, WCAG 2.2 AA contrast, and non-color state indicators.
- Keep motion purposeful and within 120–220ms; respect `prefers-reduced-motion`.

## Code Style

- Avoid redundant comments.
- Add comments only when they clarify non-obvious intent or constraints, and keep them concise.

## Tests

- Test core behavior and important risk boundaries.
- Avoid large numbers of low-value, repetitive, or implementation-detail tests.
- Skip tests when they do not provide meaningful confidence.

## Git

- Keep commit messages concise.

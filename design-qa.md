# Mobile Navigation Design QA

## Comparison Setup

- Source visual truth: `/tmp/codex-remote-attachments/01a04d81-6b9d-7701-8337-7c67960ba4fb/16612E6F-56C0-448D-89A6-BE3C1AFC33CA/1-照片-1.jpg`
- Implementation screenshot: `design-qa/implementation-mobile.png`
- Full-view comparison: `design-qa/mobile-comparison.png`
- Focused navigation comparison: `design-qa/mobile-bottom-nav-comparison.png`
- State: Library active, light theme, mobile bottom navigation visible.
- Viewport: 393 × 852 CSS px at device pixel ratio 1.
- Source pixels: 590 × 1280. Normalized to 393 × 852 with Lanczos scaling for comparison.
- Implementation pixels: 393 × 852. No density normalization required.
- QA scope: the existing screen remains the product baseline; the requested change replaces only the mobile bottom navigation with a floating liquid-glass treatment and removes More.

## Browser Evidence

- Browser-rendered navigation bounds: x 12, y 776, width 369, height 64 CSS px.
- Visible destinations: Workspace, Memos, Library, Tasks.
- Each visible navigation target is 87.75 × 52 CSS px.
- Computed material: `blur(24px) saturate(1.8)` with a 24px radius.
- More trigger count: 0.
- Horizontal overflow: none.
- Primary interaction tested: selected Tasks, observed Tasks as `aria-current`, then returned to Library and observed Library as `aria-current`.
- Console errors: none.

## Required Fidelity Surfaces

- Fonts and typography: the existing system UI stack is preserved. Labels remain 11px with a stronger selected weight, and no wrapping or truncation is present at 393px.
- Spacing and layout rhythm: the navigation floats 12px from the horizontal edges and safe bottom area. The 64px surface and 52px item targets preserve a compact iOS-like rhythm while exceeding the 44px touch minimum.
- Colors and visual tokens: the surface, border, shadow, selected tint, text, and focus color are derived from the existing semantic card, foreground, accent, primary, muted, and ring tokens.
- Image quality and asset fidelity: no raster assets are required for this component. Existing Lucide navigation icons remain sharp and consistent with the configured project icon library.
- Copy and content: More is absent. The four requested primary labels remain concise and unchanged. Archive remains available in non-mobile navigation and is intentionally excluded from the mobile bar.

## Findings

- No remaining P0, P1, or P2 findings.

## Comparison History

1. Initial pass found a P2 duplicate selection indicator: the desktop active-item rail remained visible inside the new mobile selected capsule.
2. Fixed by suppressing the desktop active pseudo-element at the mobile breakpoint.
3. Post-fix evidence in `design-qa/mobile-bottom-nav-comparison.png` shows one clear selected capsule without the redundant rail.

## Open Questions

- None.

## Implementation Checklist

- [x] Remove the More trigger and dropdown.
- [x] Keep four equal-width primary mobile destinations.
- [x] Add floating safe-area spacing and a translucent blurred surface.
- [x] Preserve visible focus treatment and 44px-plus targets.
- [x] Verify navigation interaction, mobile overflow, build, types, and tests.

## Follow-up Polish

- No P3 follow-up is required for the requested scope.

final result: passed

---

# Task UI design verification

final result: blocked

## Reference and scope

- Source: `/home/ruofeng/.codex/generated_images/01a070d6-b533-7d13-8765-15fd157de116/exec-56111399-53c6-47e0-8c09-2316fd2904d7.png`.
- Implementation: `/tasks` and the workspace Todo rail; the existing navigation and monochrome theme are retained, with a red semantic high-priority flag as requested in the follow-up annotations.
- Target viewports: desktop 1440 × 1024, tablet 834 × 1194, mobile 390 × 844, and narrow mobile 320px wide.
- Target states: idle list, inline title editing, task properties, calendar, completed group, loading and failed saves.
- Implementation screenshot: unavailable. No browser-control or screenshot tool is exposed in this session. The existing development server responds at `http://127.0.0.1:5173/tasks`, but HTTP availability is not visual verification.
- Source/implementation density normalization, full-view comparison and focused-region comparison: not performed.

## Functional decisions

- Title edits submit only the title. Enter saves, Escape cancels, and leaving the row saves; failed drafts remain editable. Composition Enter does not submit.
- The properties popover contains only target date and two icon-only priority choices. Normal tasks have no priority label.
- Date shortcuts use the workspace calendar date. This week means Sunday of the current Monday–Sunday week, including today when it is Sunday.
- Dates are optional. New tasks do not set a clock time. Existing times survive title, priority and nonempty date changes; clearing the date clears its dependent time.
- Description editing and confirmed deletion remain available through the separate details icon.
- Tasks uses a single open list and a collapsible completed group. Todo continues to show every open task.

## Automated checks

- `pnpm check`: zero errors and warnings.
- `pnpm test`: 107 tests passed across 21 files.
- `pnpm build`: passed, including production CSS verification.

## Verification limits

The automated component checks cover attribute updates, optional dates, failed saves, title focus and keyboard behavior, creation, completion rollback and confirmed deletion. They do not prove browser layout, touch behavior, contrast, or visual fidelity.

Fonts/typography, spacing/layout, colors/tokens, icon rendering and app copy all still require comparison against a browser-rendered implementation. Browser console errors have not been inspected. There is no visual comparison history and no visual pass is claimed.

## Remaining visual checks

- Capture the list and properties at matched desktop dimensions and compare with the source.
- Check title wrapping and metadata density in the narrow Todo rail.
- Verify mobile keyboard behavior, calendar bounds, touch targets and lack of horizontal overflow.
- Check popover positioning, focus return and nested deletion confirmation in a real browser.

## Task annotation follow-up

- Date shortcuts and the no-date action now use labeled icons without visible option text.
- Priority uses an outline flag for no priority and a red filled flag for high priority.
- The sort control is removed; the existing default target-date order remains.
- Updated validation: type checking and production build passed; all 11 affected task component tests passed.
- Browser-rendered verification of these updates remains unavailable.

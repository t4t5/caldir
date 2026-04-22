# caldir website

The marketing + docs site for caldir, at caldir.org.

## Stack

- Astro 5 (static, no SSR).
- Tailwind CSS v4 — design tokens live in `src/styles/global.css` inside the `@theme` block (`--color-*`, `--font-*`, `--breakpoint-*`). No `tailwind.config.js`.
- No React. Everything is `.astro` components.
- Shiki for code highlighting — theme set in `astro.config.mjs` under `markdown.shikiConfig`.

## Breakpoint

`md:` is 880px (not the Tailwind default 768px), set via `--breakpoint-md: 880px` in `global.css`. Anything matching a raw `@media (min-width: 880px)` / `(max-width: 879px)` should stay in sync.

## Docs content

- Docs are an Astro content collection at `src/content/docs/*.md`.
- Nav order and labels are driven by `src/data/docs-links.ts` — each entry has an explicit `href`.
- The Overview (`what-is-caldir`) is rendered at `/` by `src/pages/index.astro`. `/docs/what-is-caldir` 301s to `/` and is filtered out of `[...slug].astro`.
- `Layout.astro` is the shared shell (desktop two-column + mobile hamburger). Pages pass `currentSlug` so the sidebar can highlight the active link.

## Icons

- Icons live in `src/icons/` as `.astro` components (e.g. `logo.astro`, `github.astro`, `chevron-left.astro`).
- Use `stroke="currentColor"` / `fill="currentColor"` so the parent's text color drives them. The caller sets color via Tailwind on a wrapping element (e.g. `text-logo`, `text-text-muted hover:text-text-body`).
- Components accept a `class` prop forwarded to the root `<svg>`. Size comes from the caller (`class="size-6"` etc.), not the component.

## Version label

The sidebar's `v{version}` is read at build time from `../caldir-cli/Cargo.toml` inside `DocsSidebar.astro`. Bumping the CLI version automatically updates the site on rebuild.

## Layout structure

- `Layout.astro` — `<html>`, `<head>`, mobile + desktop shells, prev/next nav, footer. Used by every docs page and the homepage.
- `DocsSidebar.astro` — desktop-only sticky sidebar (logomark + nav + version + GitHub link).
- `Footer.astro` — shared footer.

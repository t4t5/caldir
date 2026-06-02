# caldir website

The marketing + docs site for caldir, at caldir.org. Static Astro 5 with Tailwind v4.

## Conventions

- Design tokens (`--color-*`, `--font-*`, `--breakpoint-*`) live in the `@theme` block in `src/styles/global.css`. There is no `tailwind.config.js`.
- The `md:` breakpoint is **880px**, not Tailwind's default 768px. Anything matching a raw `@media (min-width: 880px)` should stay in sync.
- Docs are an Astro content collection; nav order and labels come from `src/data/docs-links.ts`.
- The sidebar's `v{version}` is read at build time from `../caldir-cli/Cargo.toml` — bumping the CLI version updates the site automatically on rebuild.

## Icons

`.astro` SVG components in `src/icons/`. They use `currentColor` so the parent's text color drives them, and they accept a forwarded `class` prop for sizing — never set size inside the icon component.

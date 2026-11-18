# site

The suite's public results site: a leaderboard, a page per report, test and operation, and badges,
built as static files for GitHub Pages in the Springcraft Alt-1 design system.

## Overview

Vite and React render every page to HTML at build time; there is no server. A page that needs
interaction opts into hydration, and only that page ships JavaScript.

- **Routes are files.** Every `*.page.tsx` under [`src/pages/`](src/pages/) is a page and every
  `*.<ext>.ts` an endpoint, such as [`badges/[kernel].svg.ts`](src/pages/badges/[kernel].svg.ts). A
  directory is a path segment and `[name]` a parameter. [`routes.ts`](src/site/routes.ts) holds the
  rules and [`page.ts`](src/site/page.ts) the module contracts. Adding a page never edits a shared
  file.
- **All parsing and derivation is in [`results.ts`](src/data/results.ts):** the report types, the
  1e-7 pass rate, tiers and grades, the speed factor, the leaderboard and the test and operation
  indexes. It reads the flat report `brep-conformance-report --flat` writes, as
  [formats.md](../doc/formats.md) describes it. [`source.ts`](src/data/source.ts) only finds and
  reads the files.
- **[`Shell.tsx`](src/components/Shell.tsx)** wraps every page: the one-row chrome and the
  navigation that pages join themselves. The shared components are exported from
  [`src/components/`](src/components/index.ts), and their styles and the grade scale's tokens are in
  [`site.css`](src/styles/site.css).
- **The docs are the suite's own.** `/docs/` renders the suite's `doc/` files as one page, in the
  order [`source.ts`](src/docs/source.ts) lists them, with each function's example from
  [api.md](../doc/api.md) beside it. A link between docs goes to its place on that page, else to the
  file on GitHub.
- **A page with a little behaviour and no React** exports `enhance()` instead of `hydrate`: its
  script runs on the rendered page and its props are not shipped.
- **A kernel's license** is not in its report, so [`kernels.ts`](src/data/kernels.ts) states it; a
  kernel joining the leaderboard adds a line there.
- The design system comes from its published build at a pinned path, fetched and checked against
  pinned hashes on `npm install` by [`fetch-ds.mjs`](scripts/fetch-ds.mjs), and re-exported by
  [`ds.ts`](src/ds.ts).

## Setup

Node 22 or later.

```bash
cd site
npm install
```

## Usage

```bash
npm run dev     # http://localhost:5173, every route rendered on request
npm run build   # dist/site: the static site
npm run preview # serve dist/site
npm test        # data and routing tests, and a render of every route
npm run check   # types, tests, formatting and the build
```

`public/og.png` is the image a link to the site previews with, and every page's share tags point to
it, absolute under `SITE_URL` (the Pages build sets it). It is the leaderboard's look, not its
results, so it never names a kernel. It is not committed: the Pages build draws it from
[`scripts/og.html`](scripts/og.html) with headless Chrome, and `node scripts/og.mjs` draws it
locally (set `CHROME` where it is not in the usual place).

The site reads `<suite>/<kernel>/<version>/report.json` under `BREP_REPORTS_DIR`, and has no results
when it is unset. The tests read two reports for invented kernels in
[`src/fixtures/reports/`](src/fixtures/reports/). `SITE_BASE` sets the path the site is served
under, such as `/brep-conformance/`. The dev server reads the reports once, so restart it after they
change.

The operations pages and each test's definition read [api.md](../doc/api.md) at build time;
`BREP_API_MD` names another copy.

To add a page at `/about/`:

1. Create `src/pages/about.page.tsx`. Its default export renders the page body inside the shell.
   Export `nav = { label, order }` to join the chrome, and `hydrate = true` if it needs interaction.
2. If it needs data, or renders one URL per parameter, create `about.data.ts` beside it. `entries()`
   returns each URL's `params`, `props` and `title`, and may read `site()` from `source.ts`. The
   page imports only the `Props` type from it.

A page module is loaded in the browser when it hydrates, so it never imports `source.ts`. Link with
`url` and `href` from [`href.ts`](src/site/href.ts) so links work under `SITE_BASE`.

## Deployment

`npm run build` writes `dist/site`, which GitHub Pages serves as it is.
[`pages.yml`](../.github/workflows/pages.yml) checks, builds and deploys it on every push to `main`.
It builds with no `BREP_REPORTS_DIR` yet, so the site says no results are published. The reports,
and a check that every asset a report names exists, are not set up yet.

## Additional Information

See [doc/intro.md](doc/intro.md).

import { renderToString } from 'react-dom/server'
import type { Manifest } from 'vite'
import { Shell, type NavItem } from '@/components/Shell'
import { site } from '@/data/source'
import { href } from '@/site/href'
import type { DataModule, EndpointModule, PageEntry, PageModule, Params } from '@/site/page'
import {
  dataModuleOf,
  fileOf,
  matchRoute,
  pathOf,
  routeOf,
  sortRoutes,
  type Route,
} from './site/routes'

const pages = import.meta.glob<PageModule>('./pages/**/*.page.tsx', { eager: true })
const data = import.meta.glob<DataModule>('./pages/**/*.data.ts', { eager: true })
const endpoints = import.meta.glob<EndpointModule>(
  ['./pages/**/*.*.ts', '!./pages/**/*.data.ts', '!./pages/**/*.test.ts'],
  { eager: true },
)

const CLIENT_ENTRY = 'src/entry-client.tsx'
const DEV_HIDE = 'dev-hide'
const NOT_FOUND = './pages/404.page.tsx'
const TYPES: Record<string, string> = {
  html: 'text/html; charset=utf-8',
  svg: 'image/svg+xml',
  json: 'application/json',
  xml: 'application/xml',
  txt: 'text/plain; charset=utf-8',
}

export interface Rendered {
  path: string
  file: string
  status: number
  type: string
  body: string
}

export const routes: Route[] = sortRoutes(
  [...Object.keys(pages), ...Object.keys(endpoints)].filter(m => m !== NOT_FOUND).map(routeOf),
)

interface Entry {
  params: Params
  props: Record<string, unknown>
  title: string
  description?: string
}

function entriesOf(route: Route): Entry[] {
  const raw: (PageEntry | { params?: Params; props: Record<string, unknown> })[] =
    route.kind === 'endpoint'
      ? endpoints[route.module]!.entries()
      : (data[dataModuleOf(route.module)]?.entries() ?? [
          { props: {}, title: pages[route.module]!.title ?? '' },
        ])
  return raw.map(e => ({ title: '', ...e, params: e.params ?? {} }))
}

function navItems(): NavItem[] {
  return routes
    .filter(r => r.kind === 'page' && !r.params.length && pages[r.module]!.nav)
    .map(r => ({ ...pages[r.module]!.nav!, path: r.pattern }))
    .sort((a, b) => a.order - b.order)
}

const esc = (s: string) =>
  s.replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!)

// Where the site is served, for the absolute links a link preview needs; the Pages build sets it.
// Unset, as in dev, the links stay relative to the site.
const SITE_URL = process.env.SITE_URL
// The share image, on the CDN under a name that changes with it, so it is cached for good.
const SHARE_IMAGE = 'https://cdn.springcraft.ai/springrep/brep-kernel-leaderboard-mwljdymv.png'

// What a link to a page previews with: its title and description, and the site's share image.
function share(title: string, description: string | undefined, path: string): string[] {
  const at = (p: string) => (SITE_URL ? new URL(p.replace(/^\//, ''), SITE_URL).href : href(p))
  const meta = (key: string, value: string) => `<meta property="${key}" content="${esc(value)}">`
  return [
    meta('og:type', 'website'),
    meta('og:site_name', 'brep-conformance'),
    meta('og:title', title),
    description ? meta('og:description', description) : '',
    meta('og:url', at(path)),
    meta('og:image', SHARE_IMAGE),
    meta('og:image:width', '1200'),
    meta('og:image:height', '630'),
    meta(
      'og:image:alt',
      'brep-conformance: the B-rep kernel leaderboard, with bars of cases by the tightest tolerance passed',
    ),
    '<meta name="twitter:card" content="summary_large_image">',
  ]
}

function cssOf(manifest: Manifest, keys: string[]): string[] {
  const css = new Set<string>()
  const seen = new Set<string>()
  const walk = (key: string) => {
    const chunk = manifest[key]
    if (!chunk || seen.has(key)) return
    seen.add(key)
    chunk.css?.forEach(c => css.add(c))
    chunk.imports?.forEach(walk)
  }
  keys.forEach(walk)
  return [...css]
}

function renderPage(module: string, entry: Entry, path: string, manifest: Manifest | null): string {
  const mod = pages[module]!
  const pageHtml = renderToString(<mod.default {...entry.props} />)
  const shell = renderToString(
    <Shell path={path} nav={navItems()} suite={site().suite}>
      <div id="page" dangerouslySetInnerHTML={{ __html: pageHtml }} />
    </Shell>,
  )
  const pageKey = 'src/' + module.slice(2)
  const head: string[] = []
  const tail: string[] = []
  const boot = JSON.stringify({ module, props: mod.hydrate ? entry.props : undefined })
  tail.push(
    `<script type="application/json" id="page-data">${boot.replace(/</g, '\\u003c')}</script>`,
  )
  if (!manifest) {
    // In dev the styles arrive through the client entry, after first paint; hide the page until
    // they are in, so a page load does not flash unstyled. entry-client.tsx removes this.
    head.push(`<style id="${DEV_HIDE}">body{visibility:hidden}</style>`)
    tail.push(`<script type="module" src="${href(CLIENT_ENTRY)}"></script>`)
  } else {
    for (const css of cssOf(manifest, [CLIENT_ENTRY, pageKey]))
      head.push(`<link rel="stylesheet" href="${href(css)}">`)
    if (mod.hydrate || mod.enhance) {
      head.push(`<link rel="modulepreload" href="${href(manifest[pageKey]!.file)}">`)
      tail.push(`<script type="module" src="${href(manifest[CLIENT_ENTRY]!.file)}"></script>`)
    }
  }
  const title = entry.title ? `${entry.title} · brep-conformance` : 'brep-conformance'
  return [
    '<!doctype html>',
    '<html lang="en">',
    '<head>',
    '<meta charset="utf-8">',
    '<meta name="viewport" content="width=device-width, initial-scale=1">',
    // The ground before any stylesheet loads, so moving between pages never shows white.
    '<meta name="color-scheme" content="dark">',
    '<style>html{background:#000}</style>',
    `<title>${esc(title)}</title>`,
    entry.description ? `<meta name="description" content="${esc(entry.description)}">` : '',
    ...share(title, entry.description, path),
    `<link rel="icon" href="${href('/favicon.svg')}" type="image/svg+xml">`,
    ...head,
    '</head>',
    `<body>${shell}${tail.join('')}</body>`,
    '</html>',
  ].join('\n')
}

function rendered(route: Route, entry: Entry, path: string, manifest: Manifest | null) {
  if (route.kind === 'endpoint') {
    const ext = path.split('.').at(-1)!
    const body = endpoints[route.module]!.render(entry.props)
    return { path, file: fileOf(path), status: 200, type: TYPES[ext] ?? 'text/plain', body }
  }
  const body = renderPage(route.module, entry, path, manifest)
  return { path, file: fileOf(path), status: 200, type: TYPES.html!, body }
}

function notFound(path: string, manifest: Manifest | null): Rendered | null {
  if (!pages[NOT_FOUND]) return null
  const entry = { params: {}, props: {}, title: pages[NOT_FOUND].title ?? 'not found' }
  const body = renderPage(NOT_FOUND, entry, path, manifest)
  return { path, file: '404.html', status: 404, type: TYPES.html!, body }
}

// Renders one URL, for the dev server. `url` includes the base path.
export function render(url: string, manifest: Manifest | null = null): Rendered | null {
  const base = import.meta.env.BASE_URL
  const pathname = new URL(url, 'http://x').pathname
  if (!pathname.startsWith(base)) return null
  const path = '/' + pathname.slice(base.length)
  for (const route of routes) {
    const params = matchRoute(route, path)
    if (!params) continue
    const entry = entriesOf(route).find(e => route.params.every(k => e.params[k] === params[k]))
    if (entry) return rendered(route, entry, path, manifest)
  }
  if (path.endsWith('/') || !path.includes('.')) return notFound(path, manifest)
  return null
}

// Every file the site is made of, for the static build.
export function prerender(manifest: Manifest | null): Rendered[] {
  const out: Rendered[] = []
  const files = new Map<string, string>()
  for (const route of routes) {
    for (const entry of entriesOf(route)) {
      const path = pathOf(route, entry.params)
      const r = rendered(route, entry, path, manifest)
      const prev = files.get(r.file)
      if (prev) throw new Error(`${r.file}: rendered by ${prev} and ${route.module}`)
      files.set(r.file, route.module)
      out.push(r)
    }
  }
  const missing = notFound('/404.html', manifest)
  if (missing) out.push(missing)
  return out
}

import { describe, expect, it } from 'vitest'
import { site } from '@/data/source'
import { prerender, render, routes } from './entry-server'
import { operations } from './pages/operations/doc'

describe('the site', () => {
  const files = prerender(null)
  const byFile = new Map(files.map(f => [f.file, f]))

  it('discovers the five page routes from src/pages', () => {
    expect(routes.map(r => r.pattern)).toEqual(
      expect.arrayContaining([
        '/',
        '/reports/[suite]/[kernel]/[version]/',
        '/tests/[op]/[test]/',
        '/operations/[op]/',
        '/badges/[kernel].svg',
      ]),
    )
  })

  it('renders a page for every report, test and operation, and a badge per kernel', () => {
    const s = site()
    expect(byFile.has('index.html')).toBe(true)
    expect(byFile.has('404.html')).toBe(true)
    for (const r of s.reports) {
      const { kernel, version } = r.identity
      const at = `reports/${r.methodology.suite}/${kernel}/${version}/index.html`
      expect(byFile.has(at)).toBe(true)
    }
    for (const t of s.tests)
      expect(byFile.has(`tests/${t.operation}/${t.slug}/index.html`)).toBe(true)
    for (const o of operations().operations)
      expect(byFile.has(`operations/${o.id}/index.html`)).toBe(true)
    for (const k of s.kernels) expect(byFile.get(`badges/${k}.svg`)?.type).toBe('image/svg+xml')
  })

  it('wraps every page in the shell and names its module for the client', () => {
    for (const f of files.filter(f => f.file.endsWith('.html'))) {
      expect(f.body).toContain('class="bc-chrome"')
      expect(f.body).toMatch(/id="page-data">\{"module":"\.\/pages\//)
    }
    expect(byFile.get('index.html')!.body).toContain('aria-current="page"')
  })

  it('embeds no props for a page that does not hydrate', () => {
    expect(byFile.get('404.html')!.body).toMatch(
      /id="page-data">\{"module":"\.\/pages\/404\.page\.tsx"\}/,
    )
  })

  it('gives the home page its own title and what a shared link shows', () => {
    const home = byFile.get('index.html')!.body
    expect(home).toContain(
      '<title>Open conformance test suite for B-rep CAD kernels | brep-conformance</title>',
    )
    expect(home).toMatch(/<meta name="description" content="An open conformance test suite/)
    expect(home).toContain(
      '<meta property="og:title" content="brep-conformance, an open test suite for B-rep CAD kernels">',
    )
    expect(home).toMatch(
      /<meta property="og:description" content="Every kernel is checked the same/,
    )
  })

  it('shows a page’s own title and description in a shared link, and the site’s where it has none', () => {
    const docs = byFile.get('docs/index.html')!.body
    expect(docs).toContain('<meta property="og:title" content="docs · brep-conformance">')
    expect(docs).not.toMatch(/og:description" content="Every kernel is checked/)
    expect(byFile.get('404.html')!.body).toMatch(
      /og:description" content="Every kernel is checked the same way/,
    )
  })

  it('renders one URL for the dev server, and 404 for an unknown page', () => {
    expect(render(`/badges/${site().kernels[0]}.svg`)?.status).toBe(200)
    expect(render('/reports/0.0.0/nobody/0/')?.status).toBe(404)
    expect(render('/src/main.ts')).toBeNull()
  })
})

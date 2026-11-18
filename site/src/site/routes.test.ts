import { describe, expect, it } from 'vitest'
import { dataModuleOf, fileOf, matchRoute, pathOf, routeOf, sortRoutes } from './routes'

describe('routeOf', () => {
  it('maps page files to directory URLs', () => {
    expect(routeOf('./pages/index.page.tsx').pattern).toBe('/')
    expect(routeOf('./pages/methodology.page.tsx').pattern).toBe('/methodology/')
    const r = routeOf('./pages/reports/[kernel]/[version]/index.page.tsx')
    expect(r).toMatchObject({ kind: 'page', pattern: '/reports/[kernel]/[version]/' })
    expect(r.params).toEqual(['kernel', 'version'])
  })

  it('maps endpoint files to files with their own extension', () => {
    expect(routeOf('./pages/badges/[kernel].svg.ts')).toMatchObject({
      kind: 'endpoint',
      pattern: '/badges/[kernel].svg',
      params: ['kernel'],
    })
  })

  it('rejects files that are neither', () => {
    expect(() => routeOf('./pages/index.data.ts')).toThrow()
    expect(() => routeOf('./pages/helper.tsx')).toThrow()
  })

  it('pairs a page with its data module', () => {
    expect(dataModuleOf('./pages/reports/[report]/[kernel]/index.page.tsx')).toBe(
      './pages/reports/[report]/[kernel]/index.data.ts',
    )
  })
})

describe('matchRoute and pathOf', () => {
  const report = routeOf('./pages/reports/[kernel]/[version]/index.page.tsx')
  const badge = routeOf('./pages/badges/[kernel].svg.ts')

  it('round-trips params', () => {
    const path = pathOf(report, { kernel: 'occt', version: '8.0.1' })
    expect(path).toBe('/reports/occt/8.0.1/')
    expect(matchRoute(report, path)).toEqual({ kernel: 'occt', version: '8.0.1' })
    expect(matchRoute(badge, '/badges/arris.svg')).toEqual({ kernel: 'arris' })
  })

  it('does not match a path without the trailing slash or with extra segments', () => {
    expect(matchRoute(report, '/reports/occt/8.0.1')).toBeNull()
    expect(matchRoute(report, '/reports/occt/8.0.1/x/')).toBeNull()
    expect(matchRoute(badge, '/badges/arrisXsvg')).toBeNull()
  })

  it('refuses a param that would add a segment', () => {
    expect(() => pathOf(report, { kernel: 'a/b', version: '1' })).toThrow()
  })
  it('refuses a param that would climb out of its directory', () => {
    expect(() => pathOf(report, { kernel: '..', version: '1' })).toThrow()
  })

  it('matches and writes a parameter that spans segments', () => {
    const compare = routeOf('./pages/reports/[report]/compare/[...path]/index.page.tsx')
    expect(compare.params).toEqual(['report', 'path'])
    const params = { report: '1.0.0', path: 'booleans/degenerate/equal cylinders' }
    const path = pathOf(compare, params)
    expect(path).toBe('/reports/1.0.0/compare/booleans/degenerate/equal%20cylinders/')
    expect(matchRoute(compare, path)).toEqual(params)
    expect(matchRoute(compare, '/reports/1.0.0/compare/')).toBeNull()
    expect(() => pathOf(compare, { report: '1.0.0', path: 'booleans//x' })).toThrow()
    expect(() => pathOf(compare, { report: '1.0.0', path: 'booleans/../x' })).toThrow()
  })

  it('writes the file a link to an escaped param reaches once the host decodes it', () => {
    const path = pathOf(report, { kernel: 'k', version: '1.2.0+abc def' })
    expect(path).toBe('/reports/k/1.2.0%2Babc%20def/')
    expect(fileOf(path)).toBe('reports/k/1.2.0+abc def/index.html')
    expect(matchRoute(report, path)).toEqual({ kernel: 'k', version: '1.2.0+abc def' })
  })
})

describe('fileOf', () => {
  it('writes directories as index.html and endpoints as themselves', () => {
    expect(fileOf('/')).toBe('index.html')
    expect(fileOf('/tests/boolean/x/')).toBe('tests/boolean/x/index.html')
    expect(fileOf('/badges/occt.svg')).toBe('badges/occt.svg')
  })
})

describe('sortRoutes', () => {
  it('puts a literal segment ahead of a parameter in the same place', () => {
    const sorted = sortRoutes([
      routeOf('./pages/reports/[kernel]/index.page.tsx'),
      routeOf('./pages/reports/latest/index.page.tsx'),
    ])
    expect(sorted.map(r => r.pattern)).toEqual(['/reports/latest/', '/reports/[kernel]/'])
  })

  it('puts a parameter of one segment ahead of one that spans segments', () => {
    const sorted = sortRoutes([
      routeOf('./pages/reports/[report]/[...path]/index.page.tsx'),
      routeOf('./pages/reports/[report]/[kernel]/index.page.tsx'),
    ])
    expect(sorted.map(r => r.pattern)).toEqual([
      '/reports/[report]/[kernel]/',
      '/reports/[report]/[...path]/',
    ])
  })
})

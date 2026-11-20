import { describe, expect, it } from 'vitest'
import {
  ASSET_BASE,
  caseTimes,
  compareRuns,
  compareVersions,
  latestByKernel,
  latestReports,
  leaderboard,
  parseReport,
  speedFactors,
  summarize,
  testIndex,
} from './results'

type RawCase = Record<string, unknown>

// A flat report as `brep-conformance-report --flat` writes it.
function raw(kernel: string, version: string, cases: RawCase[], suite = '0.9.0', unix = 1) {
  return {
    kernel: { name: kernel, version, tolerance: 1e-7, configuration: 'default' },
    suite: { version: suite, contract: '0.8' },
    run: { unix, verified: false },
    environment: { os: 'linux', arch: 'x86_64', cores: 8, threads: 4 },
    summary: { cases: cases.length, pass_1e7: 0, results: {} },
    unsupported: ['torus'],
    stand_ins: { torus: 'revolve' },
    cases,
  }
}

// `result` and `tier` as the cases were written before the flat format: a tier reached is the
// row's result, a pass with none is 1e-7, anything else is the outcome.
function row(id: string, test: string, result: string, tier: string | null, time_ms = 1): RawCase {
  const r = tier && tier !== 'fail' ? tier : result === 'pass' ? '1e-7' : result
  return {
    id,
    test,
    operation: test.split('/')[0],
    kind: 'exact',
    result: r,
    time_ms,
    group_median_ms: time_ms,
    group_low_confidence: true,
  }
}

describe('parseReport', () => {
  it('reads the flat report into camelCase rows with grade and slug', () => {
    const r = parseReport(
      raw('k', '1.0.0', [
        { ...row('a', 'booleans/overlap/boxes', 'fail', '1e-5'), failure: 'measurement' },
      ]),
    )
    const c = r.cases[0]!
    expect(c.slug).toBe('overlap-boxes')
    expect(c.operation).toBe('booleans')
    expect(c.grade).toBe('1e-5')
    expect(c.tier).toBe('1e-5')
    expect(c.failure).toBe('measurement')
    expect(c.timeMs).toBe(1)
    expect(c.checks).toEqual([])
    expect(r.identity.adapter).toBeNull()
    expect(r.identity.configuration).toBe('default')
    expect(r.unsupported).toEqual(['torus'])
    expect(r.standIns).toEqual({ torus: 'revolve' })
    expect(r.environment.date).toBe('1970-01-01T00:00:01.000Z')
    expect(r.methodology.assetBase).toBe(`${ASSET_BASE}/0.9.0/k/1.0.0`)
  })

  it('names each file a case exports under its own kernel’s base', () => {
    const key = 'booleans/overlap/boxes/a.abc'
    const drew = { glb: `${key}.glb`, step: [`${key}.main.step`, `${key}.alt.step`] }
    const r = parseReport(
      raw('k', '1.0.0', [
        { ...row('a', 'booleans/overlap/boxes', 'pass', '1e-7'), exports: drew },
        { ...row('b', 'booleans/overlap/boxes', 'crash', null), exports: { glb: null, step: [] } },
        row('c', 'booleans/overlap/boxes', 'pass', '1e-7'),
      ]),
    )
    const base = `${ASSET_BASE}/0.9.0/k/1.0.0/${key}`
    expect(r.cases.map(c => [c.glb, c.steps])).toEqual([
      [`${base}.glb`, [`${base}.main.step`, `${base}.alt.step`]],
      [null, []],
      [null, []],
    ])
  })

  it('takes the folder a report names as `assets`, and no files where it names none', () => {
    const cases = [
      { ...row('a', 't/x', 'pass', '1e-7'), exports: { glb: 't/x/a.abc.glb', step: [] } },
    ]
    const named = { ...raw('k', '2.1', cases), assets: `${ASSET_BASE}/0.9.0/k/2.1/` }
    expect(parseReport(named).methodology.assetBase).toBe(`${ASSET_BASE}/0.9.0/k/2.1`)
    expect(parseReport(named).cases[0]!.glb).toBe(`${ASSET_BASE}/0.9.0/k/2.1/t/x/a.abc.glb`)
    const none = parseReport({ ...raw('k', '2.1', cases), assets: null })
    expect([none.methodology.assetBase, none.cases[0]!.glb]).toEqual([null, null])
  })

  it('loads a report’s files from a local copy of the CDN, still checking `assets`', () => {
    const cases = [
      { ...row('a', 't/x', 'pass', '1e-7'), exports: { glb: 't/x/a.abc.glb', step: [] } },
    ]
    const named = { ...raw('k', '2.1', cases), assets: `${ASSET_BASE}/0.9.0/k/2.1/` }
    const local = '/site/local-assets'
    const glb = parseReport(named, 'report', local).cases[0]!.glb
    expect(glb).toBe('/site/local-assets/0.9.0/k/2.1/t/x/a.abc.glb')
    const elsewhere = { ...named, assets: 'https://example.com/k/' }
    expect(() => parseReport(elsewhere, 'report', local)).toThrow(/report\.assets/)
  })

  it('refuses `assets` that is not the report’s own folder on the suite’s CDN', () => {
    for (const assets of [
      'https://files.example/brep-conformance/0.9.0/k/2.1/',
      `${ASSET_BASE}.evil.example/0.9.0/k/2.1/`,
      `${ASSET_BASE}/0.9.0/other/2.1/`,
      `${ASSET_BASE.replace('https:', 'http:')}/0.9.0/k/2.1/`,
      `${ASSET_BASE}/0.9.0/k/2.1`,
      7,
    ]) {
      const bad = { ...raw('k', '2.1', []), assets }
      expect(() => parseReport(bad)).toThrow(/report\.assets/)
    }
  })

  it('takes a key of plain names, a plus sign among them, and no other', () => {
    const glbOf = (glb: string) =>
      parseReport(raw('k', '1', [{ ...row('a', 't/x', 'pass', '1e-7'), exports: { glb } }]))
    expect(glbOf('t/a+b.abc.glb').cases[0]!.glb).toMatch(/\/t\/a\+b\.abc\.glb$/)
    for (const glb of ['t/_a.abc.glb', 't/.a.abc.glb', 't//a.abc.glb'])
      expect(() => glbOf(glb)).toThrow(/cases\[0\]\.exports\.glb/)
  })

  it('refuses an exported key that leaves the kernel’s files', () => {
    for (const glb of ['../other/1.0.0/t/a.abc.glb', 'https://x.example/a.glb', 't/a.abc.step']) {
      const bad = raw('k', '1', [{ ...row('a', 't/x', 'pass', '1e-7'), exports: { glb } }])
      expect(() => parseReport(bad)).toThrow(/cases\[0\]\.exports\.glb/)
    }
  })

  it('names the field it cannot read', () => {
    const bad = raw('k', '1', [{ ...row('a', 't/x', 'maybe', null) }])
    expect(() => parseReport(bad)).toThrow(/cases\[0\]\.result/)
  })

  it('takes a reached tier as the grade, else the outcome', () => {
    const r = parseReport(
      raw('k', '1', [row('a', 't/x', 'fail', 'fail'), row('b', 't/x', 'unsupported', null)]),
    )
    expect(r.cases.map(c => [c.grade, c.tier])).toEqual([
      ['fail', null],
      ['unsupported', null],
    ])
  })
})

describe('summarize', () => {
  it('counts only 1e-7 passes toward the headline, everything else against it', () => {
    const r = parseReport(
      raw('k', '1', [
        row('a', 't/a', 'pass', '1e-7'),
        row('b', 't/a', 'fail', '1e-6'),
        row('c', 't/a', 'unsupported', null),
        row('d', 't/a', 'refused', null),
        row('e', 't/a', 'timeout', null),
        row('f', 't/a', 'fail', 'fail'),
        row('g', 't/a', 'pass', '1e-7'),
        row('h', 't/a', 'crash', null),
      ]),
    )
    const s = summarize(r.cases)
    expect(s.total).toBe(8)
    expect(s.passing).toBe(2)
    expect(s.passRate).toBe(0.25)
    expect(s.grades['1e-6']).toBe(1)
    expect(s.grades.unsupported).toBe(1)
  })
})

describe('latestReports', () => {
  it('orders a version that is not a release number by when it ran', () => {
    const reports = [raw('s', 'beef', [], '0.9.0', 2), raw('s', 'abcd', [], '0.9.0', 1)]
    expect(latestReports(reports.map(r => parseReport(r)))[0]!.identity.version).toBe('beef')
  })

  it('keeps each kernel’s newest release on the current suite only', () => {
    const reports = [
      raw('a', '0.9.0', []),
      raw('a', '0.10.0', []),
      raw('a', '2.0.0', [], '0.8.0'),
      raw('b', '1.0.0', [], '0.9.0'),
    ].map(r => parseReport(r))
    const latest = latestReports(reports)
    expect(latest.map(r => `${r.identity.kernel} ${r.identity.version}`)).toEqual([
      'a 0.10.0',
      'b 1.0.0',
    ])
    expect(compareVersions('0.10.0', '0.9.0')).toBe(1)
  })
})

describe('compareRuns and latestByKernel', () => {
  it('orders release numbers by number and anything else by when it ran', () => {
    expect(compareRuns({ version: '0.10.0', date: '1' }, { version: '0.9.0', date: '2' })).toBe(1)
    expect(compareRuns({ version: 'aaaa', date: '2' }, { version: 'ffff', date: '1' })).toBe(1)
  })

  it('keeps a kernel whose newest run is on an older suite', () => {
    const reports = [
      raw('a', '1.0.0', [], '0.10.0'),
      raw('b', '1.0.0', [], '0.9.0'),
      raw('b', '2.0.0', [], '0.9.0'),
    ].map(r => parseReport(r))
    expect(latestByKernel(reports).map(r => `${r.identity.kernel} ${r.identity.version}`)).toEqual([
      'a 1.0.0',
      'b 2.0.0',
    ])
    expect(latestReports(reports).map(r => r.identity.kernel)).toEqual(['a'])
  })
})

describe('caseTimes', () => {
  it('times each kernel on the cases every kernel answered, a fail included', () => {
    const a = parseReport(
      raw('a', '1', [
        row('1', 't/a', 'pass', '1e-7', 1),
        row('2', 't/a', 'fail', 'fail', 2),
        row('3', 't/a', 'pass', '1e-7', 3),
        row('4', 't/a', 'pass', '1e-7', 100),
      ]),
    )
    const b = parseReport(
      raw('b', '1', [
        row('1', 't/a', 'pass', '1e-6', 10),
        row('2', 't/a', 'pass', '1e-7', 20),
        row('3', 't/a', 'pass', '1e-7', 30),
        row('4', 't/a', 'refused', null, 1),
      ]),
    )
    const t = caseTimes([a, b])
    expect(t.a).toEqual({ median: 2, q1: 1.5, q3: 2.5, cases: 3 })
    expect(t.b).toEqual({ median: 20, q1: 15, q3: 25, cases: 3 })
  })

  it('gives no time when no case was answered by all', () => {
    const a = parseReport(raw('a', '1', [row('1', 't/a', 'refused', null)]))
    expect(caseTimes([a])).toEqual({ a: null })
  })
})

describe('speedFactors', () => {
  it('compares group medians on cases every kernel passed at one tier, warmup dropped', () => {
    const cases = (scale: number, extra: RawCase[] = []) => [
      row('w', 'g/1', 'pass', '1e-7', 1000),
      row('a', 'g/1', 'pass', '1e-7', 1 * scale),
      row('b', 'g/1', 'pass', '1e-7', 3 * scale),
      row('c', 'g/1', 'pass', '1e-7', 5 * scale),
      row('d', 'g/2', 'pass', '1e-7', 1),
      row('e', 'g/2', 'pass', '1e-7', 10),
      ...extra,
    ]
    const fast = parseReport(raw('fast', '1', cases(1)))
    const slow = parseReport(raw('slow', '1', cases(4)))
    const s = speedFactors([fast, slow])
    // g/1 medians 3 vs 12: factor 4. g/2 has one case left after the warmup: equal, factor 1.
    expect(s.fast!.factor).toBeCloseTo(1)
    expect(s.slow!.factor).toBeCloseTo(Math.sqrt(4))
    expect(s.slow!.groups).toBe(2)
  })

  it('leaves out cases the kernels reached at different tiers', () => {
    const a = parseReport(
      raw('a', '1', [row('w', 'g/1', 'pass', '1e-7'), row('x', 'g/1', 'pass', '1e-7', 1)]),
    )
    const b = parseReport(
      raw('b', '1', [row('w', 'g/1', 'pass', '1e-7'), row('x', 'g/1', 'fail', '1e-6', 9)]),
    )
    const s = speedFactors([a, b])
    expect(s.a!.factor).toBeNull()
    expect(s.b!.groups).toBe(0)
  })

  it('leaves out a group whose fastest median is below the timer resolution', () => {
    const a = parseReport(
      raw('a', '1', [row('w', 'g/1', 'pass', '1e-7'), row('x', 'g/1', 'pass', '1e-7', 0)]),
    )
    const b = parseReport(
      raw('b', '1', [row('w', 'g/1', 'pass', '1e-7'), row('x', 'g/1', 'pass', '1e-7', 200)]),
    )
    const s = speedFactors([a, b])
    expect(s.b!.groups).toBe(0)
    expect(s.b!.factor).toBeNull()
  })
})

describe('leaderboard and indexes', () => {
  const a = parseReport(
    raw('zeta', '1', [row('1', 'b/x', 'pass', '1e-7'), row('2', 'b/x', 'pass', '1e-7')]),
  )
  const b = parseReport(
    raw('alpha', '1', [row('1', 'b/x', 'pass', '1e-7'), row('2', 'b/x', 'unsupported', null)]),
  )

  it('ranks by pass rate alone', () => {
    expect(leaderboard([b, a]).map(r => r.kernel)).toEqual(['zeta', 'alpha'])
  })

  it('lists a test’s cases from every kernel, each once, in the suite’s order', () => {
    const first = parseReport(raw('alpha', '1', [row('a', 't/x', 'pass', '1e-7')]))
    const second = parseReport(
      raw('beta', '1', [row('b', 't/x', 'fail', 'fail'), row('a', 't/x', 'pass', '1e-7')]),
    )
    expect(testIndex([first, second])[0]!.cases).toEqual(['b', 'a'])
  })

  it('places a case one kernel skipped where the suite runs it, whichever report comes first', () => {
    const cs = ['1', '2', '3'].map(id => row(id, 't/x', 'pass', '1e-7'))
    const skipped = parseReport(raw('alpha', '1', [cs[0]!, cs[2]!]))
    const all = parseReport(raw('beta', '1', cs))
    expect(testIndex([skipped, all])[0]!.cases).toEqual(['1', '2', '3'])
    expect(testIndex([all, skipped])[0]!.cases).toEqual(['1', '2', '3'])
  })

  it('gives each kernel its worst case’s grade on a test', () => {
    const [t] = testIndex([a, b])
    expect(t!.slug).toBe('x')
    expect(t!.kernels.zeta!.grade).toBe('1e-7')
    expect(t!.kernels.alpha!.grade).toBe('unsupported')
  })
})

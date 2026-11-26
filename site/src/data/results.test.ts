import { describe, expect, it } from 'vitest'
import {
  ASSET_BASE,
  buildSite,
  caseTime,
  compareRuns,
  compareVersions,
  leaderboard,
  parseReport,
  runSegment,
  selectedRuns,
  suiteView,
  summarize,
  testIndex,
  versionSegment,
} from './results'

type RawCase = Record<string, unknown>

// A flat report as `brep-conformance-report --flat` writes it.
function raw(
  kernel: string,
  version: string,
  cases: RawCase[],
  suite = '0.9.0',
  unix = 1,
  run = '1-1',
  machine: Record<string, string> = { instance_type: 'm1', ami_id: 'ami-1', kernel: '7.0' },
) {
  return {
    kernel: { name: kernel, version, tolerance: 1e-7, configuration: 'default' },
    suite: { version: suite, contract: '0.8', commit: 'c0ffee0' },
    run: { id: run, unix, verified: false, complete: true, calibration_ms: 20 },
    environment: { os: 'linux', arch: 'x86_64', cores: 8, threads: 4, machine },
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

  it('loads a report’s files from a local copy of its folder, still checking `assets`', () => {
    const cases = [
      { ...row('a', 't/x', 'pass', '1e-7'), exports: { glb: 't/x/a.abc.glb', step: [] } },
    ]
    const named = { ...raw('k', '2.1', cases), assets: `${ASSET_BASE}/0.9.0/k/2.1/` }
    const local = '/site/local-assets/0.9.0/k/2.1'
    expect(parseReport(named).cases[0]!.glb).toBe(`${ASSET_BASE}/0.9.0/k/2.1/t/x/a.abc.glb`)
    expect(parseReport(named, 'report', local).cases[0]!.glb).toBe(`${local}/t/x/a.abc.glb`)
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

describe('compareRuns', () => {
  it('orders release numbers by number and anything else by when it ran', () => {
    expect(compareRuns({ version: '0.10.0', date: '1' }, { version: '0.9.0', date: '2' })).toBe(1)
    expect(compareRuns({ version: 'aaaa', date: '2' }, { version: 'ffff', date: '1' })).toBe(1)
    expect(compareVersions('0.10.0', '0.9.0')).toBe(1)
  })
})

describe('suiteView', () => {
  const a1 = parseReport(raw('a', '1.0.0', [], '0.9.0', 10, '10-1'))
  const b1 = parseReport(raw('b', '1.0.0', [], '0.9.0', 10, '10-1'))
  const b2 = parseReport(raw('b', '2.0.0', [], '0.9.0', 20, '11-1'))
  const board = (entries: [string, string][]) => new Map(entries)

  it('draws the leaderboard reports.json names, and marks a row from another run', () => {
    const v = suiteView(
      [a1, b1, b2],
      '0.9.0',
      board([
        ['a', '10-1'],
        ['b', '11-1'],
      ]),
      '10-1',
    )
    expect(v.latest.map(r => `${r.identity.kernel} ${r.identity.version}`)).toEqual([
      'a 1.0.0',
      'b 2.0.0',
    ])
    expect(v.leaderboard.map(r => [r.kernel, r.run])).toEqual([
      ['a', '10-1'],
      ['b', '11-1'],
    ])
  })

  it('refuses a row from a run on another machine or image than the board’s own', () => {
    const other = { instance_type: 'm1', ami_id: 'ami-2', kernel: '7.0' }
    const elsewhere = parseReport(raw('b', '2.0.0', [], '0.9.0', 20, '12-1', other))
    expect(() =>
      suiteView(
        [a1, elsewhere],
        '0.9.0',
        board([
          ['a', '10-1'],
          ['b', '12-1'],
        ]),
        '10-1',
      ),
    ).toThrow(/ami_id ami-2 is not run 10-1's ami-1/)
  })

  it('refuses a row whose report did not run every test', () => {
    const partial = raw('a', '1.0.0', [], '0.9.0', 10, '13-1')
    partial.run.complete = false
    expect(() =>
      suiteView([parseReport(partial)], '0.9.0', board([['a', '13-1']]), '13-1'),
    ).toThrow(/did not run every test/)
  })

  it('gives a release run twice one page of its own, and each run a page for good', () => {
    const again = parseReport(raw('a', '1.0.0', [], '0.9.0', 30, '14-1'))
    expect(selectedRuns([a1, again], []).map(r => r.run.id)).toEqual(['14-1'])
    // The leaderboard's run of a release is its page even when a newer run of it exists.
    expect(selectedRuns([a1, again], [a1]).map(r => r.run.id)).toEqual(['10-1'])
    expect([versionSegment(a1), runSegment(a1), runSegment(again)]).toEqual([
      'a@1.0.0',
      'a@1.0.0~10-1',
      'a@1.0.0~14-1',
    ])
  })

  it('shows the suite the home page names, not the newest with a report', () => {
    const newer = parseReport(raw('a', '1.0.0', [], '1.0.0', 40, '15-1'))
    const boards = new Map([['0.9.0', { run: '10-1', kernels: board([['a', '10-1']]) }]])
    const s = buildSite([a1, newer], boards, '0.9.0')
    expect(s.suite).toBe('0.9.0')
    expect(s.suites.map(v => [v.suite, v.latest.length])).toEqual([
      ['1.0.0', 0],
      ['0.9.0', 1],
    ])
  })
})

describe('caseTime', () => {
  it('times a kernel on its own passed cases only', () => {
    const a = parseReport(
      raw('a', '1', [
        row('1', 't/a', 'pass', '1e-7', 1),
        row('2', 't/a', 'fail', 'fail', 2),
        row('3', 't/a', 'pass', '1e-7', 3),
        row('4', 't/a', 'pass', '1e-7', 100),
        row('5', 't/a', 'refused', null, 1000),
      ]),
    )
    expect(caseTime(a)).toEqual({ median: 3, q1: 2, q3: 51.5, cases: 3, step: null })
  })

  it('gives the round trips through STEP their own median, over passed cases that ran one', () => {
    const a = parseReport(
      raw('a', '1', [
        { ...row('1', 't/a', 'pass', '1e-7', 1), step_ms: 10 },
        { ...row('2', 't/a', 'pass', '1e-7', 2), step_ms: 30 },
        row('3', 't/a', 'pass', '1e-7', 3),
        { ...row('4', 't/a', 'fail', 'fail', 4), step_ms: 1000 },
      ]),
    )
    expect(a.cases[0]!.stepMs).toBe(10)
    expect(a.cases[2]!.stepMs).toBeNull()
    expect(caseTime(a)).toEqual({ median: 2, q1: 1.5, q3: 2.5, cases: 3, step: 20 })
  })

  it('gives no time when the kernel passed no case', () => {
    const a = parseReport(raw('a', '1', [row('1', 't/a', 'refused', null)]))
    expect(caseTime(a)).toBeNull()
  })
})

describe('leaderboard and indexes', () => {
  const a = parseReport(
    raw('zeta', '1', [row('1', 'b/x', 'pass', '1e-7'), row('2', 'b/x', 'pass', '1e-7')]),
  )
  const b = parseReport(
    raw('alpha', '1', [row('1', 'b/x', 'pass', '1e-7'), row('2', 'b/x', 'unsupported', null)]),
  )

  it('ranks by pass rate first', () => {
    expect(leaderboard([b, a]).map(r => r.kernel)).toEqual(['zeta', 'alpha'])
  })

  it('breaks a tie in pass rate by speed, faster first, then by name', () => {
    const passed = (kernel: string, ms: number) =>
      parseReport(raw(kernel, '1', [row('1', 'b/x', 'pass', '1e-7', ms)]))
    const none = parseReport(raw('aardvark', '1', [row('1', 'b/x', 'refused', null)]))
    const slow = passed('alpha', 9)
    const fast = passed('zeta', 2)
    const level = passed('beta', 9)
    expect(leaderboard([slow, level, fast]).map(r => r.kernel)).toEqual(['zeta', 'alpha', 'beta'])
    // A kernel that passed nothing has no time and no rate: it sorts last by rate already.
    expect(leaderboard([none, slow]).map(r => r.kernel)).toEqual(['alpha', 'aardvark'])
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

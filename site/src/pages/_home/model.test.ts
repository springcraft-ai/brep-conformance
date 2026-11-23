import { describe, expect, it } from 'vitest'
import { fmt } from '@/components'
import { GRADES, summarize, type CaseRow, type Grade, type TestEntry } from '@/data/results'
import {
  bandGrades,
  boardRows,
  caseTitle,
  distinctFilters,
  shortLicense,
  drawn,
  grid,
  keep,
  matchesQuery,
  underPrefix,
  shownShare,
  relativeTimes,
  toggleCompare,
  type Cell,
  type GridTest,
} from './model'

function cases(...grades: Grade[]): CaseRow[] {
  return grades.map((grade, i) => ({ grade, kind: 'exact', failure: null, id: `c${i}` }) as CaseRow)
}

function entry(operation: string, slug: string, byKernel: Record<string, Grade[]>): TestEntry {
  const kernels = Object.fromEntries(
    Object.entries(byKernel).map(([k, gs]) => [
      k,
      { grade: gs[0]!, summary: summarize(cases(...gs)), why: '', failure: null, medianMs: 2 },
    ]),
  )
  return { test: `x/${slug}`, slug, operation, kind: 'exact', cases: [], kernels }
}

const at = (g: Grade) => GRADES.indexOf(g)

describe('grid', () => {
  const tests = [
    entry('boolean', 'a', { k1: ['1e-7', '1e-7'], k2: ['1e-7', 'unsupported'] }),
    entry('boolean', 'b', { k1: ['1e-6'], k2: ['fail'] }),
    entry('fillet', 'c', { k1: ['1e-7'] }),
  ]
  const groups = grid(tests, ['k2', 'k1'])

  it('groups tests by operation, with cells in the kernels order given', () => {
    expect(groups.map(g => [g.operation, g.tests.map(t => t.slug)])).toEqual([
      ['boolean', ['a', 'b']],
      ['fillet', ['c']],
    ])
    expect(groups[0]!.tests[0]!.cells.map(c => c!.grade)).toEqual(['unsupported', '1e-7'])
  })

  it('sums an operation’s case counts per kernel and grades it by its worst case', () => {
    const [k2, k1] = groups[0]!.cells
    expect(k2!.counts[at('1e-7')]).toBe(1)
    expect(k2!.counts[at('unsupported')]).toBe(1)
    expect(k2!.counts[at('fail')]).toBe(1)
    expect(k2!.grade).toBe('fail')
    expect(k1!.counts[at('1e-7')]).toBe(2)
    expect(k1!.grade).toBe('1e-6')
  })

  it('leaves a cell empty where a kernel has no such test', () => {
    expect(groups[1]!.tests[0]!.cells[0]).toBeNull()
    expect(groups[1]!.cells[0]).toBeNull()
  })
})

// A test with one cell per argument, each cell one case at each grade given.
function gridTest(...cells: (Grade[] | null)[]): GridTest {
  return {
    slug: 's',
    test: 'x/s',
    cells: cells.map(
      gs =>
        gs && {
          grade: gs[0]!,
          counts: GRADES.map(g => gs.filter(x => x === g).length),
          medianMs: 1,
        },
    ),
  }
}

describe('keep', () => {
  it('keeps every test under all', () => {
    expect(keep(gridTest(['1e-7'], ['1e-7']), 'all')).toBe(true)
  })

  it('tells a failure from a function not offered and from a looser tier', () => {
    const kinds = (t: GridTest) =>
      (['failed', 'unsupported', 'looser'] as const).filter(f => keep(t, f))
    expect(kinds(gridTest(['1e-7'], ['1e-7', 'crash']))).toEqual(['failed'])
    expect(kinds(gridTest(['1e-7'], ['timeout']))).toEqual(['failed'])
    expect(kinds(gridTest(['1e-7'], ['unsupported']))).toEqual(['unsupported'])
    expect(kinds(gridTest(['1e-7'], ['1e-7', '1e-5']))).toEqual(['looser'])
    expect(kinds(gridTest(['fail'], ['unsupported']))).toEqual(['failed', 'unsupported'])
  })

  it('keeps under pass only a test every kernel ran and passed in full', () => {
    expect(keep(gridTest(['1e-7'], ['1e-7', '1e-7']), 'pass')).toBe(true)
    expect(keep(gridTest(['1e-7'], ['1e-7', '1e-6']), 'pass')).toBe(false)
    expect(keep(gridTest(['1e-7'], null), 'pass')).toBe(false)
  })

  it('asks only the kernel in scope', () => {
    const t = gridTest(['1e-7'], ['fail'])
    expect([keep(t, 'failed', 0), keep(t, 'failed', 1)]).toEqual([false, true])
    expect([keep(t, 'pass', 0), keep(t, 'pass', 1)]).toEqual([true, false])
  })
})

describe('boardRows', () => {
  const summary = summarize(cases('1e-7', '1e-6'))
  const row = (kernel: string, timed: boolean) => ({
    kernel,
    segment: kernel,
    name: kernel,
    version: '1.0.0',
    suite: '0.9.0',
    date: '2026-11-09',
    adapter: { repo: `https://example.com/${kernel}-adapter/`, commit: '0'.repeat(40) },
    summary,
    time: timed ? { median: 4, q1: 3, q3: 5, cases: 2, step: null } : null,
    run: '1-1',
    override: false,
  })

  it('keeps the leaderboard order as rank and sorts a missing time last', () => {
    const rows = boardRows([row('a', true), { ...row('b', false), adapter: null }])
    expect(rows.map(r => [r.rank, r.kernel, r.adapter])).toEqual([
      [1, 'a', 'a-adapter'],
      [2, 'b', null],
    ])
    expect(rows[1]!.medianMs).toBe(Number.POSITIVE_INFINITY)
    expect(rows[0]!.medianMs).toBe(4)
    expect(rows[0]!.counts[at('1e-6')]).toBe(1)
  })

  it('states a known kernel’s license and leaves an unknown one empty', () => {
    expect(boardRows([row('solidcore', true)])[0]!.license).toBe('Apache-2.0')
    expect(boardRows([row('nope', true)])[0]!.license).toBeNull()
  })
})

describe('distinctFilters', () => {
  it('offers a filter only where it keeps some tests and not all of them', () => {
    const tests = [gridTest(['1e-7'], ['fail']), gridTest(['1e-7'], ['1e-7'])]
    expect(distinctFilters(tests)).toEqual(['all', 'failed', 'pass'])
    expect(distinctFilters([gridTest(['1e-7'], ['fail'])])).toEqual(['all'])
  })

  it('offers both of two filters that keep the same tests', () => {
    const tests = [gridTest(['fail', 'unsupported']), gridTest(['1e-7'])]
    expect(distinctFilters(tests)).toEqual(['all', 'failed', 'unsupported', 'pass'])
  })

  it('follows the kernel in scope', () => {
    const tests = [gridTest(['1e-7'], ['fail']), gridTest(['1e-7'], ['1e-7'])]
    expect(distinctFilters(tests, 0)).toEqual(['all'])
    expect(distinctFilters(tests, 1)).toEqual(['all', 'failed', 'pass'])
  })
})

describe('shownShare', () => {
  it('rounds to 100% or 0% only a share that is one', () => {
    expect(fmt.pct(shownShare(2000, 2000))).toBe('100.0%')
    expect(fmt.pct(shownShare(1999, 2000))).toBe('99.9%')
    expect(fmt.pct(shownShare(1, 2000))).toBe('0.1%')
    expect(fmt.pct(shownShare(0, 2000))).toBe('0.0%')
    expect(fmt.pct(shownShare(1194, 1253))).toBe('95.3%')
  })
})

describe('caseTitle', () => {
  it('names each grade’s count', () => {
    const counts = GRADES.map(g => (g === '1e-7' ? 3 : g === 'fail' ? 1 : 0))
    expect(caseTitle('k', 'spheres', { grade: 'fail', counts, medianMs: null })).toBe(
      'k · spheres: 3 1e-7, 1 fail of 4 cases',
    )
    expect(caseTitle('k', 'spheres', null)).toBe('k · spheres: not run')
  })
})

const cell = (pass: number, of: number, medianMs: number | null): Cell => ({
  grade: pass === of ? '1e-7' : 'fail',
  counts: GRADES.map(g => (g === '1e-7' ? pass : g === 'fail' ? of - pass : 0)),
  medianMs,
})

describe('relativeTimes', () => {
  it('divides by the fastest cell that passed a case, and skips one that passed none', () => {
    expect(relativeTimes([cell(2, 2, 4), cell(1, 2, 2), cell(0, 2, 1), null])).toEqual([
      2,
      1,
      null,
      null,
    ])
  })
})

describe('toggleCompare', () => {
  const order = ['a', 'b', 'c', 'd', 'e']

  it('adds in rank order up to four, and removes down to none', () => {
    expect(toggleCompare(['a', 'c'], 'b', order)).toEqual(['a', 'b', 'c'])
    expect(toggleCompare(['a', 'b', 'c', 'd'], 'e', order)).toEqual(['a', 'b', 'c', 'd'])
    expect(toggleCompare(['a', 'b', 'c'], 'b', order)).toEqual(['a', 'c'])
    expect(toggleCompare(['a', 'c'], 'c', order)).toEqual(['a'])
    expect(toggleCompare(['c'], 'c', order)).toEqual([])
    expect(toggleCompare([], 'd', order)).toEqual(['d'])
  })
})

describe('matchesQuery', () => {
  const t: GridTest = { slug: 'overlap-spheres', test: 'booleans/overlap/spheres', cells: [] }

  it('needs every word somewhere in the operation or test', () => {
    expect(matchesQuery(t, 'booleans', 'sphere bool')).toBe(true)
    expect(matchesQuery(t, 'booleans', 'sphere fillet')).toBe(false)
    expect(matchesQuery(t, 'booleans', '  ')).toBe(true)
  })
})

describe('underPrefix', () => {
  it('keeps a test at or under the level, not one that only starts with the same letters', () => {
    expect(underPrefix('booleans/degenerate/a', 'booleans/degenerate')).toBe(true)
    expect(underPrefix('booleans/degenerate/a', 'booleans/degenerate/a')).toBe(true)
    expect(underPrefix('booleans/degenerated/a', 'booleans/degenerate')).toBe(false)
  })
})

describe('drawn', () => {
  const at = (g: Grade) => GRADES.indexOf(g)

  it('draws 1e-6 with 1e-5 and 1e-3 with the failures, keeping every case', () => {
    const counts = GRADES.map((_, i) => i + 1)
    const out = drawn(counts)
    expect(out[at('1e-6')]).toBe(0)
    expect(out[at('1e-3')]).toBe(0)
    expect(out[at('1e-5')]).toBe(counts[at('1e-5')]! + counts[at('1e-6')]!)
    expect(out[at('fail')]).toBe(counts[at('fail')]! + counts[at('1e-3')]!)
    expect(out.reduce((a, b) => a + b, 0)).toBe(counts.reduce((a, b) => a + b, 0))
  })
})

describe('bandGrades', () => {
  it('lists the grades a band draws, in GRADES order', () => {
    expect(bandGrades('1e-7')).toEqual(['1e-7'])
    expect(bandGrades('1e-5')).toEqual(['1e-6', '1e-5'])
    expect(bandGrades('fail')).toEqual(['1e-3', 'fail', 'crash'])
    expect(bandGrades('crash')).toEqual(['1e-3', 'fail', 'crash'])
    expect(bandGrades('refused')).toEqual(['refused', 'timeout'])
  })
})

describe('shortLicense', () => {
  it('sets each registered kind of expression in a few words', () => {
    expect(shortLicense('MIT OR Apache-2.0')).toBe('MIT / Apache-2.0')
    expect(shortLicense('AGPL-3.0-or-later AND BSD-3-Clause AND MIT')).toBe(
      'AGPL-3.0+ & BSD-3-Clause & MIT',
    )
    expect(shortLicense('LGPL-2.1-only WITH OCCT-exception-1.0')).toBe('LGPL-2.1 + exception')
    expect(shortLicense('Apache-2.0')).toBe('Apache-2.0')
  })
})

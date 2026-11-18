import { describe, expect, it } from 'vitest'
import { TIERS, type CaseRow, type Grade, type Kind, type Tier } from '@/data/results'
import {
  byOperation,
  duration,
  filterFromLocation,
  filterFromSearch,
  filterToLocation,
  filterToSearch,
  gradeSet,
  groupByTest,
  matches,
  NO_FILTER,
  openingFilter,
  ordinal,
  packCases,
  reasonOf,
  sameFilter,
  shortCase,
  sortRows,
  unpackCases,
} from './cases'

function row(
  id: string,
  test: string,
  operation: string,
  result: 'pass' | Grade,
  tier: Tier | 'fail' | null,
  timeMs: number | null,
  why = '',
  kind: Kind = 'exact',
): CaseRow {
  const grade: Grade = tier && tier !== 'fail' ? tier : result === 'pass' ? '1e-7' : result
  return {
    id,
    test,
    slug: test.split('/').slice(1).join('-'),
    operation,
    kind,
    result: grade,
    tier: (TIERS as readonly string[]).includes(grade) ? (grade as Tier) : null,
    grade,
    why,
    timeMs,
    groupMedianMs: null,
    groupLowConfidence: true,
    asset: 'abc',
    assetStep: false,
    failure: null,
    checks: [],
  }
}

const CASES = [
  row('a1', 'booleans/overlap/boxes', 'boolean', 'pass', '1e-7', 12),
  row('a2', 'booleans/overlap/boxes', 'boolean', 'pass', '1e-6', 30),
  row('b1', 'booleans/overlap/spheres', 'boolean', 'fail', 'fail', 8, 'B9 8 points misplaced'),
  row('c1', 'fillets/conic/edge', 'fillet_conic', 'unsupported', null, null, 'not offered'),
  row('c2', 'fillets/conic/edge', 'fillet_conic', 'unsupported', null, null, 'not offered'),
  row('d1', 'fillets/round/edge', 'fillet', 'pass', '1e-7', 5, '', 'sound'),
  row('d2', 'fillets/round/face', 'fillet', 'unsupported', null, null, 'not offered'),
  row('e1', 'reject/bad/input', 'boolean', 'timeout', null, 120000, '', 'rejection'),
]

describe('packCases', () => {
  it('round-trips every field the table shows, in the report order', () => {
    const rows = unpackCases(packCases(CASES))
    expect(rows).toHaveLength(CASES.length)
    rows.forEach((r, i) => {
      const c = CASES[i]!
      expect(r).toMatchObject({
        order: i,
        id: c.id,
        test: c.test,
        slug: c.slug,
        operation: c.operation,
        kind: c.kind,
        grade: c.grade,
        why: c.why,
        timeMs: c.timeMs,
      })
    })
  })

  it('stores each test and reason once', () => {
    const packed = packCases(CASES)
    expect(packed.tests).toHaveLength(6)
    expect(packed.whys).toEqual(['', 'B9 8 points misplaced', 'not offered'])
  })
})

describe('matches', () => {
  const rows = unpackCases(packCases(CASES))
  const ids = (f: Partial<typeof NO_FILTER>) =>
    rows.filter(r => matches(r, { ...NO_FILTER, ...f })).map(r => r.id)

  it('keeps everything with no filter', () => {
    expect(ids({})).toHaveLength(CASES.length)
  })

  it('filters by grade, with "below" as everything short of 1e-7', () => {
    expect(ids({ grade: 'unsupported' })).toEqual(['c1', 'c2', 'd2'])
    expect(ids({ grade: 'below' })).toEqual(['a2', 'b1', 'c1', 'c2', 'd2', 'e1'])
  })

  it('filters by a set of grades, keeping the cases of any of them', () => {
    const one = (g: Grade) => ids({ grade: g })
    const both = ids({ grade: gradeSet(['fail', 'unsupported']) })
    expect([...both].sort()).toEqual([...one('unsupported'), ...one('fail')].sort())
  })

  it('combines operation, kind and grade', () => {
    expect(ids({ operation: 'fillet' })).toEqual(['d1', 'd2'])
    expect(ids({ operation: 'fillet', grade: '1e-7' })).toEqual(['d1'])
    expect(ids({ kind: 'rejection' })).toEqual(['e1'])
  })

  it('keeps one test by its whole path', () => {
    const one = CASES.find(c => c.id === 'd2')!.test
    expect(ids({ test: one })).toEqual(CASES.filter(c => c.test === one).map(c => c.id))
  })

  it('searches test, case id and reason, every word must hit', () => {
    expect(ids({ q: 'SPHERES' })).toEqual(['b1'])
    expect(ids({ q: 'c2' })).toEqual(['c2'])
    expect(ids({ q: 'offered face' })).toEqual(['d2'])
    expect(ids({ q: '   ' })).toHaveLength(CASES.length)
  })
})

describe('sortRows', () => {
  const rows = unpackCases(packCases(CASES))

  it('ranks grades worst first when descending, keeping report order in ties', () => {
    expect(sortRows(rows, 'grade', 'desc').map(r => r.id)).toEqual([
      'b1',
      'e1',
      'c1',
      'c2',
      'd2',
      'a2',
      'a1',
      'd1',
    ])
  })

  it('puts cases with no time last in both directions', () => {
    expect(sortRows(rows, 'time', 'asc').map(r => r.id)).toEqual([
      'd1',
      'b1',
      'a1',
      'a2',
      'e1',
      'c1',
      'c2',
      'd2',
    ])
    expect(
      sortRows(rows, 'time', 'desc')
        .map(r => r.id)
        .slice(0, 2),
    ).toEqual(['e1', 'a2'])
    expect(
      sortRows(rows, 'time', 'desc')
        .map(r => r.id)
        .slice(-3),
    ).toEqual(['c1', 'c2', 'd2'])
  })

  it('does not reorder its input', () => {
    sortRows(rows, 'test', 'desc')
    expect(rows.map(r => r.order)).toEqual(CASES.map((_, i) => i))
  })
})

describe('the query string', () => {
  it('round-trips a filter', () => {
    const f = {
      grade: 'fail',
      operation: 'boolean',
      test: 'booleans/overlap/spheres',
      kind: 'sound',
      q: 'b9 points',
    } as const
    expect(filterFromSearch(filterToSearch(f))).toEqual(f)
    expect(filterToSearch(NO_FILTER)).toBe('')
  })

  it('ignores values it does not know', () => {
    expect(filterFromSearch('?grade=great&kind=fuzzy')).toEqual(NO_FILTER)
    expect(filterFromSearch('?grade=fail,great')).toEqual(NO_FILTER)
  })

  it('reads a set of grades in GRADES order, and round-trips it', () => {
    expect(filterFromSearch('?grade=crash,fail').grade).toBe('fail,crash')
    const f = { ...NO_FILTER, grade: gradeSet(['1e-3', 'fail', 'crash']) }
    expect(f.grade).toBe('1e-3,fail,crash')
    expect(filterFromSearch(filterToSearch(f))).toEqual(f)
  })
})

describe('groupByTest', () => {
  const rows = unpackCases(packCases(CASES))

  it('gathers the shown rows by test, in the order each test first comes', () => {
    const groups = groupByTest(sortRows(rows, 'grade', 'desc'), rows)
    expect(groups.map(g => g.test)).toEqual([
      ...new Set(sortRows(rows, 'grade', 'desc').map(r => r.test)),
    ])
    expect(groups.reduce((n, g) => n + g.rows.length, 0)).toBe(rows.length)
  })

  it('counts a test’s shown cases by grade against all of its cases', () => {
    const shown = rows.filter(r => r.grade !== '1e-7')
    for (const g of groupByTest(shown, rows)) {
      expect(g.total).toBe(rows.filter(r => r.test === g.test).length)
      expect(g.grades.reduce((n, [, c]) => n + c, 0)).toBe(g.rows.length)
      expect(g.grades.every(([grade]) => grade !== '1e-7')).toBe(true)
      expect(new Set(g.whys)).toEqual(new Set(g.rows.map(r => reasonOf(r.why)).filter(Boolean)))
      expect(g.reasoned).toBe(g.rows.filter(r => reasonOf(r.why)).length)
    }
  })
})

describe('reasonOf', () => {
  it('reads two reasons that differ only in the entities they name as one', () => {
    const a = 'a ∖ b: invalid: the shells would touch along an edge (+e1487, +f848, +f854)'
    const b = 'a ∖ b: invalid: the shells would touch along an edge (+e1604, +f878, +f884)'
    expect(reasonOf(a)).toBe('a ∖ b: invalid: the shells would touch along an edge')
    expect(reasonOf(a)).toBe(reasonOf(b))
  })

  it('drops an entity named in the sentence, and keeps a parenthesis of words', () => {
    expect(reasonOf('no closed form for +f143 (plane surface) against +e251 (line curve)')).toBe(
      'no closed form for (plane surface) against (line curve)',
    )
  })

  it('leaves a reason that names no entity as it is', () => {
    expect(reasonOf('B9 8 points misplaced')).toBe('B9 8 points misplaced')
  })
})

describe('byOperation', () => {
  it('counts every case once under its operation, by grade', () => {
    const rows = unpackCases(packCases(CASES))
    const ops = byOperation(rows)
    expect(ops.map(o => o.operation)).toEqual([...new Set(rows.map(r => r.operation))].sort())
    expect(ops.reduce((n, o) => n + o.total, 0)).toBe(rows.length)
    for (const o of ops)
      expect(o.passing).toBe(
        rows.filter(r => r.operation === o.operation && r.grade === '1e-7').length,
      )
  })
})

describe('ordinal', () => {
  it('says a place as it is spoken', () => {
    expect([1, 2, 3, 4, 11, 12, 13, 21, 22, 103].map(ordinal)).toEqual([
      '1st',
      '2nd',
      '3rd',
      '4th',
      '11th',
      '12th',
      '13th',
      '21st',
      '22nd',
      '103rd',
    ])
  })
})

describe('duration', () => {
  it('names the unit a reader would', () => {
    expect(duration(840)).toBe('840 ms')
    expect(duration(12_400)).toBe('12.4 s')
    expect(duration(425_000)).toBe('7 min 05 s')
    expect(duration(8_040_000)).toBe('2 h 14 min')
  })
})

describe('the opening filter', () => {
  const below = openingFilter(3)

  it('opens on the cases that fall short, and on every case where none does', () => {
    expect(below.grade).toBe('below')
    expect(openingFilter(0)).toEqual(NO_FILTER)
  })

  it('reads an empty query string as the opening view and writes that view as none', () => {
    expect(filterFromLocation('', below)).toEqual(below)
    expect(filterToLocation(below, below)).toBe('')
  })

  it('writes the whole table as its own query string, and reads it back', () => {
    expect(filterToLocation(NO_FILTER, below)).toBe('?grade=all')
    expect(filterFromLocation('?grade=all', below)).toEqual(NO_FILTER)
    expect(filterToLocation(NO_FILTER, NO_FILTER)).toBe('')
  })

  it('reads a query string that names no filter as the opening view', () => {
    expect(filterFromLocation('?utm_source=x', below)).toEqual(below)
  })

  it('takes a search of spaces alone as no search', () => {
    expect(sameFilter({ ...NO_FILTER, q: '  ' }, NO_FILTER)).toBe(true)
    expect(filterToLocation({ ...below, q: ' ' }, below)).toBe('')
    expect(sameFilter({ ...NO_FILTER, q: 'box' }, NO_FILTER)).toBe(false)
  })

  it('keeps a link that names a test on every case of it', () => {
    expect(filterFromLocation('?test=a/b', below)).toEqual({ ...NO_FILTER, test: 'a/b' })
  })
})

describe('shortCase', () => {
  it('gives the first six characters of the id’s hash', () => {
    expect(shortCase('box-box-7fe85639399e5b7b')).toBe('7fe856')
  })
})

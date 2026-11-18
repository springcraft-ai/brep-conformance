import { describe, expect, it } from 'vitest'
import { firstLeaf, narrow, tocOf, type TocPart } from './Contents'

const toc: TocPart[] = [
  {
    id: 'part-api',
    title: 'API',
    groups: [
      { id: 'types', heading: 'Types', entries: [] },
      {
        id: 'blends',
        heading: 'Blends',
        entries: [
          { id: 'fillet', heading: 'fillet', fn: true },
          { id: 'chamfer', heading: 'chamfer', fn: true },
        ],
      },
    ],
  },
  {
    id: 'part-contract',
    title: 'The contract',
    groups: [{ id: 'tolerance', heading: 'Tolerance', entries: [] }],
  },
]

describe('tocOf', () => {
  it('keeps the headings and drops what is under them', () => {
    const parts = [
      {
        id: 'p',
        title: 'P',
        intro: 'long',
        groups: [
          {
            id: 'g',
            heading: 'G',
            body: 'long',
            entries: [{ id: 'e', heading: 'E', fn: false, body: 'long' }],
          },
        ],
      },
    ]
    expect(tocOf(parts)).toEqual([
      {
        id: 'p',
        title: 'P',
        groups: [{ id: 'g', heading: 'G', entries: [{ id: 'e', heading: 'E', fn: false }] }],
      },
    ])
  })
})

describe('narrow', () => {
  it('is the whole contents for no query, or one of spaces', () => {
    expect(narrow(toc, '')).toBe(toc)
    expect(narrow(toc, '  ')).toBe(toc)
  })

  it('keeps a heading that holds every word, with what it sits under', () => {
    expect(narrow(toc, 'FILL')).toEqual([
      {
        id: 'part-api',
        title: 'API',
        groups: [
          {
            id: 'blends',
            heading: 'Blends',
            entries: [{ id: 'fillet', heading: 'fillet', fn: true }],
          },
        ],
      },
    ])
  })

  it('keeps a group that matches without the entries that do not', () => {
    expect(narrow(toc, 'blends')[0]!.groups).toEqual([
      { id: 'blends', heading: 'Blends', entries: [] },
    ])
  })

  it('keeps nothing when no heading matches', () => {
    expect(narrow(toc, 'fillet tolerance')).toEqual([])
  })
})

describe('firstLeaf', () => {
  it('is the first heading with nothing under it', () => {
    expect(firstLeaf(narrow(toc, 'fill'))).toBe('fillet')
    expect(firstLeaf(narrow(toc, 'toler'))).toBe('tolerance')
    expect(firstLeaf(toc)).toBe('types')
    expect(firstLeaf([])).toBeNull()
  })
})

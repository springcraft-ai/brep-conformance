import { describe, expect, it } from 'vitest'
import contractMd from '../../../../../../../doc/contract.md?raw'
import apiMd from '../../../../../../../doc/api.md?raw'
import { site } from '@/data/source'
import { clausesOf, definitionOf } from './definition'

describe('definitionOf', () => {
  it('finds a section for every area but parts', () => {
    const missing = site()
      .operations.map(o => o.operation)
      .filter(op => op !== 'parts' && !definitionOf(op, apiMd))
    expect(missing).toEqual([])
  })

  it('reads the booleans’ signatures and the paragraph that defines them', () => {
    const d = definitionOf('booleans', apiMd)!
    expect(d.heading).toBe('Booleans')
    expect(d.signatures).toContain('intersection(a: Solid, b: Solid) -> Solid')
    expect(d.paragraphs[0]![0]).toMatch(/^Every boolean returns the regularized set operation/)
  })

  it('stops at the next section, leaves out tables and keeps only a link’s text', () => {
    const md =
      '## Booleans\n\nSee [b](#b).\n\n| f |\n| - |\n\n### union\n\n`union(a) -> Solid`\n\n' +
      'Joins.\n\n## Blends\n\n### fillet\n\n`fillet(s) -> Solid`'
    expect(definitionOf('booleans', md)).toEqual({
      heading: 'Booleans',
      signatures: ['union(a) -> Solid'],
      paragraphs: [['See b.']],
    })
    expect(definitionOf('parts', md)).toBeNull()
  })
})

describe('clausesOf', () => {
  it('reads every B and Q clause of the contract', () => {
    const c = clausesOf(contractMd)
    expect(c.B9).toMatch(/sample points/)
    expect(c.Q1).toBe('The kernel completes the act')
    expect(Object.keys(c).filter(k => k.startsWith('B')).length).toBeGreaterThanOrEqual(40)
  })
})

import { describe, expect, it } from 'vitest'
import { badSegment, KERNELS } from './kernels'

describe('badSegment', () => {
  it('accepts every entry of the registry', () => {
    for (const k of KERNELS) expect(badSegment(k)).toBeNull()
  })

  it('refuses a name or version a URL would encode, or one that is no folder', () => {
    for (const version of ['0.2.9 (bb09c367)', '0.2.9+bb09c367', '..', '.1', 'a/b', '']) {
      expect(badSegment({ kernel: 'k', version })).toMatch(/^version/)
    }
    expect(badSegment({ kernel: '-k', version: '1' })).toMatch(/^kernel/)
    for (const version of ['0.2.9', '8.0.1.1.0', '1.0.0-rc.1', '2026_11_20']) {
      expect(badSegment({ kernel: 'next.BREP', version })).toBeNull()
    }
  })
})

import { describe, expect, it } from 'vitest'
import type { Grade } from '@/data/results'
import {
  drawnBy,
  kernelOrder,
  point,
  stepLabel,
  shortCaseIds,
  testName,
  titleOf,
  trailOf,
  value,
  zoomKeyOf,
} from './format'

describe('zoomKeyOf', () => {
  it('names ⌘ on Apple systems and ctrl elsewhere', () => {
    expect(zoomKeyOf('macOS', 'MacIntel')).toBe('⌘')
    expect(zoomKeyOf(undefined, 'iPad')).toBe('⌘')
    expect(zoomKeyOf('Windows', 'Win32')).toBe('ctrl')
    expect(zoomKeyOf(undefined, 'Linux x86_64')).toBe('ctrl')
  })

  it('falls back to the older platform string when the newer one is empty', () => {
    expect(zoomKeyOf('', 'MacIntel')).toBe('⌘')
  })
})

describe('trailOf', () => {
  it('climbs from a test to its report through the page of each level', () => {
    expect(trailOf('1.0.0', 'booleans/degenerate/equal-cylinders-crossing')).toEqual([
      { label: 'Report 1.0.0', href: '/reports/1.0.0/' },
      { label: 'booleans', href: '/reports/1.0.0/compare/booleans/' },
      { label: 'degenerate', href: '/reports/1.0.0/compare/booleans/degenerate/' },
    ])
    expect(trailOf('1.0.0', 'fillets/box-all-edges').map(c => c.label)).toEqual([
      'Report 1.0.0',
      'fillets',
    ])
  })
})

describe('titleOf', () => {
  it('capitalizes each word but the minor ones inside the title', () => {
    expect(titleOf('booleans/coincident/thread-crest-on-a-shank-from-where-it-runs-out')).toBe(
      'Thread Crest on a Shank from Where It Runs Out',
    )
    expect(titleOf('booleans')).toBe('Booleans')
    expect(titleOf('x/a-solid-with-itself')).toBe('A Solid with Itself')
  })
})

describe('value', () => {
  it('writes counts whole, small values in exponent form, the rest to six figures', () => {
    expect(value(8)).toBe('8')
    expect(value(3700.244)).toBe('3700.24')
    expect(value(0.000776)).toBe('7.76e-4')
    expect(value(1e-7)).toBe('1.00e-7')
    expect(value(0)).toBe('0')
    expect(point([1.08, -12.7, 0.111])).toBe('1.08, -12.7, 0.111')
  })
})

describe('testName', () => {
  it('keeps only the last level and reads hyphens as spaces', () => {
    expect(testName('booleans/overlap/spheres')).toBe('spheres')
    expect(testName('modify/offset-drilled-block-hole-fills')).toBe(
      'offset drilled block hole fills',
    )
  })
})

describe('shortCaseIds', () => {
  it('cuts the hash short and drops a family every id shares', () => {
    expect(shortCaseIds(['separated-b201c5cb8261410a', 'separated-64253debd143b5c0'])).toEqual([
      'b201c5',
      '64253d',
    ])
  })

  it('keeps the family where the ids are of more than one', () => {
    expect(shortCaseIds(['box-box-1a2b3c4d5e6f7a8b', 'box-sphere-0011223344556677'])).toEqual([
      'box-box 1a2b3c',
      'box-sphere 001122',
    ])
  })
})

describe('kernelOrder', () => {
  it('ranks the kernels by their cases at each tier in turn, then by fewer failures', () => {
    const cases: (Grade | null)[][] = [
      ['1e-7', '1e-7', '1e-6'],
      ['1e-6', '1e-7', '1e-7'],
      ['fail', '1e-7', '1e-7'],
    ]
    expect(kernelOrder(cases, [0, 1, 2])).toEqual([1, 2, 0])
  })

  it('breaks a tie on the test by the leaderboard', () => {
    expect(kernelOrder([['1e-7', '1e-7', 'fail']], [2, 0, 1])).toEqual([1, 0, 2])
  })
})

describe('drawnBy', () => {
  const drew = { glb: 'x.glb' }
  const inOrder = [0, 1, 2]

  it('shows the drawing of the kernel that passed at the best tier', () => {
    expect(drawnBy([drew, drew, drew], ['fail', '1e-5', '1e-7'], inOrder, -1)).toBe(2)
    expect(drawnBy([drew, drew], ['refused', '1e-3'], [0, 1], -1)).toBe(1)
  })

  it('breaks a tie by the kernels’ order', () => {
    expect(drawnBy([drew, drew, drew], ['1e-7', 'fail', '1e-7'], [2, 1, 0], -1)).toBe(2)
    expect(drawnBy([drew, drew, drew], ['1e-7', 'fail', '1e-7'], inOrder, -1)).toBe(0)
  })

  it('keeps the reader’s kernel wherever it passed, at any tier', () => {
    expect(drawnBy([drew, drew], ['1e-7', '1e-5'], [0, 1], 1)).toBe(1)
    expect(drawnBy([drew, drew], ['1e-7', 'fail'], [0, 1], 1)).toBe(0)
    expect(drawnBy([drew, null], ['1e-7', '1e-7'], [0, 1], 1)).toBe(0)
  })

  it('shows a failing kernel’s drawing only where no kernel that passed drew the case', () => {
    expect(drawnBy([drew, null], ['fail', '1e-7'], [0, 1], -1)).toBe(0)
    expect(drawnBy([drew, drew], ['crash', 'fail'], [1, 0], -1)).toBe(1)
  })

  it('shows none where no kernel drew it', () => {
    expect(drawnBy([null, { glb: null }], ['1e-7', '1e-7'], [0, 1], 0)).toBeNull()
  })
})

describe('stepLabel', () => {
  it('names a lone file plainly, and one of several by its solid’s role', () => {
    const at = 'https://cdn.example/b/1.0.0/k/1/t/x/box-box-1f.0123456789abcdef'
    expect(stepLabel(`${at}.step`)).toBe('step')
    expect(stepLabel(`${at}.main.step`)).toBe('step · main')
    expect(stepLabel(`${at}.alt-2.step`)).toBe('step · alt-2')
  })
})

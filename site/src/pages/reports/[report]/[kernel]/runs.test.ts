import { describe, expect, it } from 'vitest'
import { parseReport, suiteView } from '@/data/results'
import { differences, segmentsOf } from './index.data'

function report(run: string, unix: number, changes: Record<string, unknown> = {}) {
  return parseReport({
    kernel: { name: 'k', version: '0.2.9' },
    suite: { version: '2.0.0', commit: 'aaaaaaa1' },
    adapter: { repo: 'https://example.com/k', commit: 'bbbbbbb1' },
    run: { id: run, unix, complete: true, calibration_ms: 35.61 },
    environment: { machine: { instance_type: 'm1', ami_id: 'ami-1', kernel: '7.0' } },
    cases: [],
    ...changes,
  })
}

describe('a release run twice', () => {
  const first = report('10-1', 100)
  const second = report('11-1', 200, {
    adapter: { repo: 'https://example.com/k', commit: 'ccccccc1' },
    run: { id: '11-1', unix: 200, complete: true, calibration_ms: 35.64 },
  })

  it('names what the other run did differently, and nothing that is the same', () => {
    expect(differences(second, first)).toEqual(['adapter bbbbbbb → ccccccc'])
    const slower = report('12-1', 300, {
      run: { id: '12-1', unix: 300, complete: true, calibration_ms: 41.2 },
    })
    expect(differences(slower, first)).toEqual(['calibration 35.6 → 41.2 ms'])
  })

  it('renders the leaderboard run at the kernel and release, and each run at its own', () => {
    const v = suiteView([first, second], '2.0.0', new Map([['k', '10-1']]), '10-1')
    expect(segmentsOf(first, v)).toEqual(['k', 'k@0.2.9', 'k@0.2.9~10-1'])
    expect(segmentsOf(second, v)).toEqual(['k@0.2.9~11-1'])
    // Without the leaderboard on it, a release's own page is its newest run.
    const off = suiteView([first, second], '2.0.0')
    expect(segmentsOf(second, off)).toEqual(['k@0.2.9', 'k@0.2.9~11-1'])
  })
})

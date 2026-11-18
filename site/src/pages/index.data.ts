import { runDays } from '@/components/format'
import type { SuiteView } from '@/data/results'
import { site } from '@/data/source'
import { SITE_NAME, SOCIAL_DESCRIPTION, SOCIAL_TITLE } from '@/site/meta'
import type { PageEntry } from '@/site/page'
import { boardRows, grid, type BoardRow, type GridGroup } from './_home/model'

export interface Props {
  // The home page, or a report's own page.
  home: boolean
  suite: string
  cases: number
  tests: number
  kernels: string[]
  // Each kernel's page in the report, in `kernels` order: kernelSegment.
  segments: string[]
  // The days the board's runs ended, earliest and latest, as ISO dates; empty with no dated run.
  runs: string[]
  board: BoardRow[]
  groups: GridGroup[]
}

// The leaderboard of one report; the home page is the current one's.
export function propsOf(v: SuiteView, home: boolean): Props {
  const kernels = v.leaderboard.map(r => r.kernel)
  return {
    home,
    suite: v.suite,
    cases: v.leaderboard[0]?.summary.total ?? 0,
    tests: v.tests.length,
    kernels,
    segments: v.leaderboard.map(r => r.segment),
    runs: runDays(v.leaderboard.map(r => r.date)),
    board: boardRows(v.leaderboard),
    groups: grid(v.tests, kernels),
  }
}

export function entries(): PageEntry<Props>[] {
  return [
    {
      props: propsOf(site(), true),
      title: 'results',
      fullTitle: `Open conformance test suite for B-rep CAD kernels | ${SITE_NAME}`,
      description:
        'An open conformance test suite for B-rep CAD kernels. Every kernel runs the same cases, ' +
        'with right answers worked out from the math. MIT licensed.',
      social: { title: SOCIAL_TITLE, description: SOCIAL_DESCRIPTION },
    },
  ]
}

import { runDays } from '@/components/format'
import { site } from '@/data/source'
import { SITE_NAME, SOCIAL_DESCRIPTION, SOCIAL_TITLE } from '@/site/meta'
import type { PageEntry } from '@/site/page'
import { boardRows, grid, type BoardRow, type GridGroup } from './_home/model'

export interface Props {
  suite: string
  cases: number
  tests: number
  kernels: string[]
  // The days the board's runs ended, earliest and latest, as ISO dates; empty with no dated run.
  runs: string[]
  board: BoardRow[]
  groups: GridGroup[]
}

export function entries(): PageEntry<Props>[] {
  const s = site()
  const kernels = s.leaderboard.map(r => r.kernel)
  return [
    {
      props: {
        suite: s.suite,
        cases: s.leaderboard[0]?.summary.total ?? 0,
        tests: s.tests.length,
        kernels,
        runs: runDays(s.leaderboard.map(r => r.date)),
        board: boardRows(s.leaderboard),
        groups: grid(s.tests, kernels),
      },
      title: 'results',
      fullTitle: `Open conformance test suite for B-rep CAD kernels | ${SITE_NAME}`,
      description:
        'An open conformance test suite for B-rep CAD kernels. Every kernel runs the same cases, ' +
        'with right answers worked out from the math. MIT licensed.',
      social: { title: SOCIAL_TITLE, description: SOCIAL_DESCRIPTION },
    },
  ]
}

import { summarize, type Report } from '@/data/results'
import { site } from '@/data/source'
import type { EndpointModule } from '@/site/page'
import { badgeText, renderBadge } from './badge'

interface Props {
  kernel: string
  passRate: number
  olderSuite: string | null
}

// One badge per kernel on any leaderboard, so an embedded badge outlives a suite release the kernel
// has not been rerun on yet: its entry on the newest suite whose leaderboard has it. On the current
// suite it is the leaderboard's headline.
const badge: EndpointModule<Props> = {
  entries: () => {
    const s = site()
    const entries = new Map<string, Report>()
    for (const v of s.suites)
      for (const r of v.latest)
        if (!entries.has(r.identity.kernel)) entries.set(r.identity.kernel, r)
    return [...entries.values()].map(r => ({
      params: { kernel: r.identity.kernel },
      props: {
        kernel: r.identity.kernel,
        passRate: summarize(r.cases).passRate,
        olderSuite: r.methodology.suite === s.suite ? null : r.methodology.suite,
      },
    }))
  },
  render: ({ kernel, passRate, olderSuite }) =>
    renderBadge(badgeText(kernel, passRate, olderSuite ?? undefined)),
}

export const { entries, render } = badge

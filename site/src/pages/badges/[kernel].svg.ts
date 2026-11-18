import { latestByKernel, summarize } from '@/data/results'
import { site } from '@/data/source'
import type { EndpointModule } from '@/site/page'
import { badgeText, renderBadge } from './badge'

interface Props {
  kernel: string
  passRate: number
  olderSuite: string | null
}

// One badge per kernel that has any report, so an embedded badge outlives a suite release the
// kernel has not been rerun on yet. On the current suite it is the leaderboard's headline.
const badge: EndpointModule<Props> = {
  entries: () => {
    const s = site()
    return latestByKernel(s.all).map(r => ({
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

import { site } from '@/data/source'
import type { EndpointModule } from '@/site/page'
import { leaderboardOf, type Leaderboard } from './leaderboard'

const leaderboard: EndpointModule<Leaderboard> = {
  entries: () => [{ props: leaderboardOf(site()) }],
  render: board => JSON.stringify(board),
}

export const { entries, render } = leaderboard

import { PageHead, Section } from '@/components'
import { useUrlQuery } from '@/components/search'
import { Grid } from '../../../../_home/Grid'
import '../../../../index.css'
import { testName, trailOf } from './format'
import type { LevelProps } from './index.data'

// The tests under one level of the suite's tree, in the leaderboard's table, open.
export function LevelPage({ suite, level, kernels, segments, shown, groups }: LevelProps) {
  const [q, setQ] = useUrlQuery()
  const pages = Object.fromEntries(kernels.map((k, i) => [k, segments[i]!]))
  return (
    <div className="fn-page hm-home">
      <PageHead trail={trailOf(suite, level)} title={testName(level)} />
      <Section label="by test">
        <Grid
          groups={groups}
          kernels={kernels}
          shown={shown}
          suite={suite}
          pages={pages}
          level={level}
          q={q}
          onSearch={setQ}
        />
      </Section>
    </div>
  )
}

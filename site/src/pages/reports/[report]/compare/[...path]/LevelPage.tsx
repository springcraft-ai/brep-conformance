import { useState } from 'react'
import { PageHead, Section } from '@/components'
import { useUrlQuery } from '@/components/search'
import { Grid } from '../../../../_home/Grid'
import { toggleCompare } from '../../../../_home/model'
import '../../../../index.css'
import { testName, trailOf } from './format'
import type { LevelProps } from './index.data'

// The tests under one level of the suite's tree, in the leaderboard's table, open.
export function LevelPage({ suite, level, kernels, segments, shown, groups }: LevelProps) {
  const [q, setQ] = useUrlQuery()
  const [picked, setPicked] = useState(shown)
  const pages = Object.fromEntries(kernels.map((k, i) => [k, segments[i]!]))
  return (
    <div className="fn-page hm-home">
      <PageHead trail={trailOf(suite, level)} title={testName(level)} />
      <Section label="by test">
        <Grid
          groups={groups}
          kernels={kernels}
          shown={picked}
          onToggleKernel={k => setPicked(p => toggleCompare(p, k, kernels))}
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

import { useEffect, useState } from 'react'
import { Annot, fmt, GradeLegend, KernelChip, PageHead, Section } from '@/components'
import { queryFromUrl, useChromeSearch } from '@/components/search'
import { EmptyState } from '@/ds'
import { GRADES, TIERS } from '@/data/results'
import { href, url } from '@/site/href'
import { Grid } from './_home/Grid'
import { TierBar } from './_home/TierBar'
import { TierBoard } from './_home/TierBoard'
import { COMPARE_MAX, defaultCompare, toggleCompare, type BoardRow } from './_home/model'
import type { Props } from './index.data'
import './index.css'

export const nav = { label: 'results', order: 0 }
export const hydrate = true

const n = (x: number) => x.toLocaleString('en-US')

// The cases a kernel passes only at a tolerance looser than 1e-7.
const looserOf = (r: BoardRow) =>
  TIERS.reduce((sum, t) => sum + (t === '1e-7' ? 0 : r.counts[GRADES.indexOf(t)]!), 0)

export default function Home({ suite, cases, tests, kernels, board, groups }: Props) {
  const versions = Object.fromEntries(board.map(r => [r.kernel, r.version]))
  const [picked, setPicked] = useState(() => defaultCompare(board))
  const [q, setQ] = useState('')

  useEffect(() => setQ(queryFromUrl()), [])
  useChromeSearch(q, v => {
    setQ(v)
    const { pathname, hash } = window.location
    const search = v.trim() ? `?q=${encodeURIComponent(v)}` : ''
    window.history.replaceState(null, '', pathname + search + hash)
    // The search box is in the header and what it narrows is a screen below: bring that up.
    const grid = document.getElementById('by-test')
    if (v.trim() && grid && grid.getBoundingClientRect().top > window.innerHeight * 0.6)
      grid.scrollIntoView({ block: 'start' })
  })

  const compare = (kernel: string) => setPicked(p => toggleCompare(p, kernel, kernels))
  const canCompare = kernels.length > COMPARE_MAX
  const timed = board.find(r => r.time)?.time?.cases ?? 0
  const speedTests = board.length ? Math.min(...board.map(r => r.speedGroups)) : 0

  // The suite and its counts come from the reports, so with none there is nothing true to state.
  if (board.length === 0)
    return (
      <div className="hm-home">
        <PageHead title="B-rep kernel leaderboard" />
        <p className="hm-tagline">
          An open test suite for exact B-rep CAD kernels. A case passes only at 1e-7 mm.
        </p>
        <EmptyState
          title="no results published yet"
          height={320}
          action={<a href={href('/docs/#introduction-add-your-kernel')}>add your kernel</a>}
        />
      </div>
    )

  return (
    <div className="hm-home">
      <PageHead
        title="B-rep kernel leaderboard"
        corner={[`suite ${suite}`, `${kernels.length} kernels`, `${n(tests)} tests`]}
      />
      <p className="hm-tagline">
        An open test suite for exact B-rep CAD kernels. Every kernel runs the same {n(cases)} cases,
        and a case passes only at 1e-7 mm.
      </p>

      <Section
        label="latest release per kernel"
        aside={<a href={href('/docs/#introduction-how-it-works')}>how scoring works</a>}
      >
        <TierBoard board={board} picked={canCompare ? picked : null} onCompare={compare} />
        <ol className="hm-cards" aria-label="Leaderboard">
          {board.map(r => (
            <li key={r.kernel} className="hm-card">
              <span className="hm-card__rank">{r.rank}</span>
              <KernelChip
                kernel={r.kernel}
                version={r.version}
                href={url.report(r.suite, r.kernel, r.version)}
              />
              <span className="hm-overall">{fmt.pct(r.rate)}</span>
              <span className="hm-card__bar">
                <TierBar
                  counts={r.counts}
                  label={`${r.kernel}: ${r.passing} of ${r.total} cases at 1e-7`}
                />
              </span>
              <Annot>
                {fmt.factor(r.speedFactor)} the fastest · median case{' '}
                {r.time ? fmt.ms(r.time.median) : '–'}
                {r.maintainers && ' · made by the suite’s maintainer'}
              </Annot>
              {looserOf(r) > 0 && (
                <Annot>
                  +{n(looserOf(r))} more {looserOf(r) === 1 ? 'case' : 'cases'} at a looser
                  tolerance
                </Annot>
              )}
            </li>
          ))}
        </ol>
        <a className="hm-slot" href={href('/docs/#introduction-add-your-kernel')}>
          + your kernel
        </a>
        <GradeLegend />
        <p className="hm-rules bc-annot">
          expected values never come from a kernel · an operation a kernel does not offer counts as
          a failure
          {timed > 0 && ` · case times are over the ${n(timed)} cases every kernel answered`}
          {speedTests > 0 &&
            ` · time vs fastest: a kernel’s time over the fastest kernel’s on each of the ${speedTests} tests all pass alike, averaged, so 1.00× means fastest on every one`}
        </p>
      </Section>

      <div id="by-test">
        <Section
          label="by test"
          aside="a test’s name opens the test · a cell opens that kernel’s cases"
        >
          <Grid
            groups={groups}
            kernels={kernels}
            shown={picked}
            suite={suite}
            versions={versions}
            q={q}
          />
        </Section>
      </div>
    </div>
  )
}

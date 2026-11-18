import { useEffect, useState } from 'react'
import { fmt, PageHead, Section } from '@/components'
import { queryFromUrl, useSearchBox } from '@/components/search'
import { EmptyState } from '@/ds'
import { href, url } from '@/site/href'
import { Grid } from './_home/Grid'
import { TierBar } from './_home/TierBar'
import { BoardKey, bests, TierBoard, within } from './_home/TierBoard'
import { COMPARE_MAX, defaultCompare, toggleCompare } from './_home/model'
import type { Props } from './index.data'
import './index.css'

export const nav = { label: 'results', order: 0 }
export const hydrate = true

const n = (x: number) => x.toLocaleString('en-US')

export default function Home({ suite, cases, kernels, runs, board, groups }: Props) {
  const versions = Object.fromEntries(board.map(r => [r.kernel, r.version]))
  const [picked, setPicked] = useState(() => defaultCompare(board))
  const [q, setQ] = useState('')

  useEffect(() => setQ(queryFromUrl()), [])
  useSearchBox(q, v => {
    setQ(v)
    const { pathname, hash } = window.location
    const search = v.trim() ? `?q=${encodeURIComponent(v)}` : ''
    window.history.replaceState(null, '', pathname + search + hash)
  })

  const compare = (kernel: string) => setPicked(p => toggleCompare(p, kernel, kernels))
  const canCompare = kernels.length > COMPARE_MAX

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

  const b = bests(board)
  return (
    <div className="hm-home">
      <PageHead title="B-rep kernel leaderboard" />

      <section className="hm-lead">
        <div className="hm-board__head">
          <p className="hm-lede">
            An open test suite for exact B-rep CAD kernels. Every kernel runs the same {n(cases)}{' '}
            cases, and a case passes only at 1e-7 mm.
          </p>
          <BoardKey board={board} />
        </div>
        <TierBoard board={board} picked={canCompare ? picked : null} onCompare={compare} />
        <ol className="hm-cards" aria-label="Leaderboard">
          {/* The board's own winners, so a phone marks the same figures. */}
          {board.map(r => (
            <li key={r.kernel} className="hm-card">
              <span className="hm-card__rank">{r.rank}</span>
              <span className="hm-card__kernel">
                <a className="hm-kname" href={url.report(r.suite, r.kernel, r.version)}>
                  {r.kernel}
                </a>
                <span className="hm-license">{fmt.version(r.version)}</span>
              </span>
              <span className="hm-card__bar">
                <TierBar
                  counts={r.counts}
                  label={`${r.kernel}: ${r.passing} of ${r.total} cases at 1e-7`}
                  top={r.rank === 1}
                />
              </span>
              <dl className="hm-card__figs">
                <div>
                  <dt>pass at 1e-7</dt>
                  <dd className={`hm-overall${r.kernel === b.rate ? ' is-best' : ''}`}>
                    {fmt.pct(r.rate)}
                  </dd>
                </div>
                <div>
                  <dt>pass at 1e-5</dt>
                  <dd className={`hm-overall${r.kernel === b.loose ? ' is-best' : ''}`}>
                    {fmt.pct(within(r, '1e-5'))}
                  </dd>
                </div>
                <div>
                  <dt>speed</dt>
                  <dd className={`hm-overall${r.kernel === b.time ? ' is-best' : ''}`}>
                    {r.time ? fmt.ms(r.time.median) : '–'}
                  </dd>
                </div>
              </dl>
            </li>
          ))}
        </ol>
        <div className="hm-foot">
          <a className="hm-slot" href={href('/docs/#introduction-add-your-kernel')}>
            + your kernel
          </a>
          <span className="hm-foot__meta bc-annot">
            <a href={href('/docs/#introduction-how-it-works')}>how scoring works</a>
            <span>
              {runs.length > 0 && `${fmt.runs(runs)} · `}
              suite {suite} · {n(cases)} cases
            </span>
          </span>
        </div>
      </section>

      <div id="by-test">
        <Section label="by test">
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

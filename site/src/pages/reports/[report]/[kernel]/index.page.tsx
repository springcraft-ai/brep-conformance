import { useEffect, useMemo, useState, type MouseEvent } from 'react'
import type { ReactNode } from 'react'
import { Annot, fmt, GradeLabel, PageHead, Section } from '@/components'
import { repoName } from '@/data/kernels'
import { GRADES, type Grade } from '@/data/results'
import { url } from '@/site/href'
import { TierBar } from '@/pages/_home/TierBar'
import {
  duration,
  filterFromSearch,
  filterToSearch,
  keeps,
  NO_FILTER,
  onlyGrade,
  ordinal,
  unpackCases,
  type Filter,
} from './cases'
import { CaseTable } from './CaseTable'
import type { Props } from './index.data'
import './report.css'

export const hydrate = true

export default function ReportPage(props: Props) {
  const { identity, environment, methodology, summary, standing } = props
  const rows = useMemo(() => unpackCases(props.cases), [props.cases])
  const [filter, setFilter] = useState<Filter>(NO_FILTER)
  const [details, setDetails] = useState(false)

  // The server renders every case; a filter in the URL applies once the page hydrates.
  useEffect(() => {
    const read = () => setFilter(filterFromSearch(window.location.search))
    read()
    window.addEventListener('popstate', read)
    return () => window.removeEventListener('popstate', read)
  }, [])

  const applyFilter = (f: Filter) => {
    setFilter(f)
    const { pathname, hash } = window.location
    window.history.replaceState(null, '', pathname + filterToSearch(f) + hash)
  }

  const show = (e: MouseEvent, f: Filter) => {
    e.preventDefault()
    applyFilter(f)
  }
  const blockFilter = (g: Grade): Filter => ({ ...filter, grade: filter.grade === g ? 'all' : g })
  const casesHref = (f: Filter) => `./${filterToSearch(f)}`

  const id = identity
  const env = environment
  const m = methodology

  return (
    <div className="fn-page rp">
      <PageHead
        trail={[{ label: `Report ${m.suite}`, href: url.report(m.suite) }]}
        title={
          <>
            {id.name} <span className="rp-version">{fmt.version(id.version)}</span>
          </>
        }
      />
      <section className="bc-section" aria-label="result">
        {/* Every case by grade in one bar across the page, and under it each grade, which is the
            table's filter. */}
        <div className="rp-headline">
          {/* A block lists its grade's cases; the same block again shows every grade. */}
          <span
            className="rp-headline__bar"
            onClick={e => {
              const seg = (e.target as Element).closest<HTMLElement>('a[data-grade]')
              if (seg) show(e, blockFilter(seg.dataset.grade as Grade))
            }}
          >
            <TierBar
              counts={GRADES.map(g => summary.grades[g])}
              label={`${summary.passing} of ${summary.total} cases at 1e-7`}
              href={g => casesHref(blockFilter(g))}
              each
            />
          </span>
          <div className="rp-grades" role="group" aria-label="show the cases of a result">
            {GRADES.filter(g => g === '1e-7' || summary.grades[g] > 0).map(g => {
              const on = keeps(filter.grade, g)
              // The one grade shown, chosen again, shows every grade.
              const next: Filter = { ...filter, grade: on && onlyGrade(filter.grade) ? 'all' : g }
              return summary.grades[g] ? (
                <a
                  key={g}
                  className="rp-grade sc-focus"
                  href={casesHref(next)}
                  onClick={e => show(e, next)}
                  aria-current={on || undefined}
                  title={on ? 'show every grade' : `show the ${summary.grades[g]} ${g} cases`}
                >
                  <GradeLabel grade={g} />
                  <span className="rp-grade__n">{summary.grades[g]}</span>
                </a>
              ) : (
                <span key={g} className="rp-grade" data-empty="">
                  <GradeLabel grade={g} />
                  <span className="rp-grade__n">0</span>
                </span>
              )
            })}
          </div>
          <p className="rp-headline__line">
            <span className="rp-headline__value">{fmt.pct(summary.passRate)}</span>
            <span>pass at 1e-7 mm, over all {summary.total} cases</span>
            {/* The suite's machine runs every kernel of a report; a run elsewhere says so here. */}
            {!env.verified && <span className="rp-unverified">unverified run</span>}
            {!props.registered && <span className="rp-unverified">no longer registered</span>}
            {standing && (
              <a className="rp-standing" href={url.report(m.suite)}>
                {ordinal(standing.place)} of {standing.of} kernels
              </a>
            )}
            <button
              type="button"
              className="rp-details__toggle sc-focus"
              aria-expanded={details}
              aria-controls="rp-details"
              onClick={() => setDetails(!details)}
            >
              {details ? '− details' : '+ details'}
            </button>
          </p>
        </div>
      </section>

      {/* What a reader needs only to look into a result: opened from the figures' line above. */}
      {details && (
        <div className="rp-details__groups" id="rp-details">
          <div>
            <Annot>kernel</Annot>
            <Specs
              rows={[
                ...(props.license ? [{ label: 'license', value: props.license }] : []),
                {
                  label: 'declared tolerance',
                  value: id.declaredTolerance === null ? '–' : fmt.sci(id.declaredTolerance, 0),
                  unit: id.declaredTolerance === null ? undefined : 'mm',
                },
                ...(id.configuration ? [{ label: 'configuration', value: id.configuration }] : []),
              ]}
            />
          </div>
          <div>
            <Annot>run</Annot>
            <Specs
              rows={[
                ...(id.adapter
                  ? [
                      {
                        label: 'adapter',
                        value: `${repoName(id.adapter.repo)} ${id.adapter.commit.slice(0, 7)}`,
                      },
                    ]
                  : []),
                ...(props.adapterLicense
                  ? [{ label: 'adapter license', value: props.adapterLicense }]
                  : []),
                ...(props.run.id ? [{ label: 'run', value: props.run.id }] : []),
                ...(props.run.suiteCommit
                  ? [{ label: 'suite commit', value: props.run.suiteCommit.slice(0, 7) }]
                  : []),
                {
                  label: 'finished',
                  value: env.date ? env.date.replace('T', ' ').replace(/\.\d+Z$/, ' UTC') : '–',
                },
                { label: 'total case time', value: duration(props.totalMs) },
              ]}
            />
            {props.releases.length > 1 && (
              <div className="rp-releases">
                <Annot>releases</Annot>
                <div className="rp-releases__list">
                  {props.releases.map(r => {
                    // A release run more than once names each run by the day it ended.
                    const reruns = props.releases.filter(
                      o => o.suite === r.suite && o.version === r.version,
                    ).length
                    const day = r.date ? ` · ${r.date.slice(0, 10)}` : ''
                    const label =
                      `${fmt.version(r.version)} · suite ${r.suite}` + (reruns > 1 ? day : '')
                    const key = `${r.suite}/${r.version}/${r.run ?? ''}`
                    const differs = r.differs.length ? (
                      <span className="rp-release__differs">{r.differs.join(' · ')}</span>
                    ) : null
                    return r.here ? (
                      <span key={key} className="rp-release" aria-current="page">
                        {label}
                      </span>
                    ) : (
                      <a key={key} className="rp-release" href={url.kernel(r.suite, r.segment)}>
                        {label}
                        {differs}
                      </a>
                    )
                  })}
                </div>
              </div>
            )}
          </div>
        </div>
      )}

      <div id="cases">
        <Section label="cases">
          <CaseTable
            rows={rows}
            suite={m.suite}
            kernel={props.shown ? id.kernel : null}
            filter={filter}
            onFilter={applyFilter}
          />
          {props.functions.length > 0 && (
            <details className="rp-fold">
              <summary>
                <span>operations not offered</span>
                <span className="bc-machine">{props.functions.length}</span>
              </summary>
              <p className="rp-fold__body rp-functions">
                {props.functions.map((f, i) => (
                  <span key={f.name}>
                    {i > 0 && ', '}
                    {f.documented ? <a href={url.operation(f.name)}>{f.name}</a> : f.name}
                    {props.standIns[f.name] && <Annot> sent as {props.standIns[f.name]}</Annot>}
                  </span>
                ))}
              </p>
            </details>
          )}
        </Section>
      </div>
    </div>
  )
}

// The run's particulars as the page's other rows are set: a small-caps label, the value right.
function Specs({ rows }: { rows: { label: string; value: ReactNode; unit?: string }[] }) {
  return (
    <dl className="rp-spec">
      {rows.map(r => (
        <div key={r.label} className="rp-spec__row">
          <dt className="bc-annot">{r.label}</dt>
          <dd>
            {r.value}
            {r.unit && <span className="rp-spec__unit"> {r.unit}</span>}
          </dd>
        </div>
      ))}
    </dl>
  )
}

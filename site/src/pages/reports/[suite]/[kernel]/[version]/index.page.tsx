import { useEffect, useMemo, useState, type MouseEvent } from 'react'
import { SpecTable } from '@/ds'
import { Annot, fmt, GradeLabel, KernelChip, PageHead, Section } from '@/components'
import { useChromeSearch } from '@/components/search'
import { repoName } from '@/data/kernels'
import { FAILURES, GRADES, KINDS, TIERS, type Grade } from '@/data/results'
import { href, url } from '@/site/href'
import {
  byOperation,
  duration,
  filterFromLocation,
  filterToLocation,
  NO_FILTER,
  openingFilter,
  ordinal,
  unpackCases,
  type Filter,
} from './cases'
import { CaseTable } from './CaseTable'
import type { Props } from './index.data'
import './report.css'

export const hydrate = true

const METHODOLOGY = href('/docs/#part-the-contract')

export default function ReportPage(props: Props) {
  const { identity, environment, methodology, summary, totalMs, standing } = props
  const rows = useMemo(() => unpackCases(props.cases), [props.cases])
  const operations = useMemo(() => byOperation(rows), [rows])
  const notPassing = summary.total - summary.passing
  const opening = useMemo(() => openingFilter(notPassing), [notPassing])
  const [filter, setFilter] = useState<Filter>(opening)

  // The server renders the opening view; a filter in the URL applies once the page hydrates.
  useEffect(() => {
    const read = () => setFilter(filterFromLocation(window.location.search, opening))
    read()
    window.addEventListener('popstate', read)
    return () => window.removeEventListener('popstate', read)
  }, [opening])

  const applyFilter = (f: Filter) => {
    setFilter(f)
    const { pathname, hash } = window.location
    window.history.replaceState(null, '', pathname + filterToLocation(f, opening) + hash)
  }

  useChromeSearch(filter.q, q => applyFilter({ ...filter, q }))

  const show = (e: MouseEvent, f: Filter) => {
    e.preventDefault()
    applyFilter(f)
    document.getElementById('cases')?.scrollIntoView()
  }
  const casesHref = (f: Filter) => `${filterToLocation(f, opening) || '?'}#cases`

  const id = identity
  const env = environment
  const m = methodology

  return (
    <>
      <PageHead
        title={
          <>
            {id.name} <span className="rp-version">{id.version}</span>
          </>
        }
        corner={[
          `suite ${m.suite}`,
          `run ${fmt.date(env.date)}${env.verified ? '' : ' · unverified'}`,
          ...(id.adapter
            ? [`adapter ${repoName(id.adapter.repo)} ${id.adapter.commit.slice(0, 7)}`]
            : []),
        ]}
      />
      {props.current && (
        <p className="rp-current">
          <Annot>not the release ranked</Annot>
          <span>
            The results page ranks{' '}
            <KernelChip
              kernel={id.kernel}
              version={props.current.version}
              href={url.report(props.current.suite, id.kernel, props.current.version)}
            />
            , its latest release on suite {props.current.suite}.
          </span>
        </p>
      )}

      <Section label="result" aside="select a grade to list its cases">
        <div className="rp-result">
          <div className="rp-headline">
            <span className="rp-headline__value">{fmt.pct(summary.passRate)}</span>
            <Annot>pass at 1e-7 mm, over all {summary.total} cases</Annot>
            {standing && (
              <a className="rp-standing" href={url.home()}>
                {ordinal(standing.place)} of {standing.of} kernels
                {standing.speedFactor !== null &&
                  (standing.speedFactor < 1.005
                    ? ' · the fastest'
                    : ` · ${fmt.factor(standing.speedFactor)} the fastest’s time`)}
              </a>
            )}
          </div>

          <dl className="rp-stats">
            {FAILURES.map(f => (
              <Stat
                key={f}
                label={`${f} failures`}
                gloss={FAILURE_GLOSS[f]}
                value={summary.failures[f]}
              />
            ))}
            {KINDS.map(k => (
              <Stat
                key={k}
                label={`${k} tests`}
                gloss={KIND_GLOSS[k]}
                value={
                  summary.kinds[k].total
                    ? `${fmt.pct(summary.kinds[k].passing / summary.kinds[k].total)}`
                    : '–'
                }
                sub={`${summary.kinds[k].passing} of ${summary.kinds[k].total}`}
              />
            ))}
            <Stat label="total case time" value={duration(totalMs)} />
          </dl>

          <div className="rp-dist" aria-label="cases by grade">
            {GRADES.filter(g => g === '1e-7' || summary.grades[g] > 0).map(g => (
              <GradeRow
                key={g}
                grade={g}
                count={summary.grades[g]}
                total={summary.total}
                href={casesHref({ ...NO_FILTER, grade: g })}
                onPick={e => show(e, { ...NO_FILTER, grade: g })}
              />
            ))}
          </div>
        </div>
      </Section>

      <Section label="by area" aside="select an area to list its cases">
        <div className="rp-ops">
          <div className="rp-ops__row rp-ops__head" aria-hidden="true">
            <span>area</span>
            <span>pass at 1e-7</span>
            <span>cases by grade</span>
            <span>short</span>
          </div>
          {operations.map(o => {
            const f: Filter = {
              ...NO_FILTER,
              operation: o.operation,
              grade: o.passing < o.total ? 'below' : 'all',
            }
            return (
              <a
                key={o.operation}
                className="rp-ops__row sc-focus"
                href={casesHref(f)}
                onClick={e => show(e, f)}
              >
                <span className="rp-ops__name">{o.operation}</span>
                <span>
                  {fmt.pct(o.passing / o.total)}
                  <span className="rp-stat__sub">
                    {o.passing} of {o.total}
                  </span>
                </span>
                <span className="rp-stack" aria-hidden="true">
                  {GRADES.map((g, i) =>
                    o.counts[i] ? (
                      <span
                        key={g}
                        className="rp-bar__fill"
                        data-grade={g}
                        title={`${o.counts[i]} ${g}`}
                        style={{ flex: `${o.counts[i]} 0 1px` }}
                      />
                    ) : null,
                  )}
                </span>
                <span className="rp-ops__short">
                  {GRADES.flatMap((g, i) =>
                    i > 0 && o.counts[i] ? [`${o.counts[i]} ${g}`] : [],
                  ).join(' · ') || '–'}
                </span>
              </a>
            )
          })}
        </div>
        {props.functions.length > 0 && (
          <details className="rp-fold">
            <summary>
              <span>operations not offered</span>
              <span className="bc-machine">{props.functions.length}</span>
            </summary>
            <p className="rp-fold__body rp-functions">
              {props.functions.map((f, i) => (
                <span key={f}>
                  {i > 0 && ', '}
                  <a href={url.operation(f)}>{f}</a>
                  {props.standIns[f] && <Annot> sent as {props.standIns[f]}</Annot>}
                </span>
              ))}
            </p>
          </details>
        )}
      </Section>

      <div id="cases">
        <Section
          label="cases"
          aside={notPassing ? 'opens on the cases short of 1e-7, by test' : 'by test'}
        >
          <CaseTable
            rows={rows}
            kernel={id.kernel}
            filter={filter}
            opening={opening}
            onFilter={applyFilter}
          />
        </Section>
      </div>

      <Section label="about this run" aside={<a href={METHODOLOGY}>how a report is scored</a>}>
        <div className="rp-specs">
          <SpecTable
            density="compact"
            rows={[
              ...(props.license ? [{ label: 'license', value: props.license }] : []),
              {
                label: 'declared tolerance',
                value: id.declaredTolerance === null ? '–' : fmt.sci(id.declaredTolerance, 0),
                unit: id.declaredTolerance === null ? undefined : 'mm',
              },
              ...(id.configuration ? [{ label: 'configuration', value: id.configuration }] : []),
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
              ...(props.reviewers
                ? [
                    {
                      label: 'adapter reviewed by',
                      value: ['the suite’s maintainers', ...props.reviewers].join(', '),
                    },
                  ]
                : []),
              { label: 'contract', value: m.contract || '–' },
              { label: 'tiers', value: TIERS.join(' · '), unit: 'mm' },
              {
                label: 'runner',
                value: env.verified ? 'the suite’s benchmark machine' : 'unverified',
              },
              {
                label: 'finished',
                value: env.date ? env.date.replace('T', ' ').replace(/\.\d+Z$/, ' UTC') : '–',
              },
            ]}
          />
          <SpecTable
            density="compact"
            rows={[
              { label: 'os', value: env.os || '–' },
              { label: 'architecture', value: env.arch || '–' },
              { label: 'cores', value: env.cores ?? '–' },
              { label: 'threads', value: env.threads ?? '–' },
            ]}
          />
        </div>
        {props.releases.length > 1 && (
          <div className="rp-releases">
            <Annot>releases</Annot>
            <div className="rp-releases__list">
              {props.releases.map(r => {
                const here = r.suite === m.suite && r.version === id.version
                return (
                  <KernelChip
                    key={`${r.suite}/${r.version}`}
                    kernel={id.kernel}
                    version={`${r.version} · suite ${r.suite}`}
                    href={here ? undefined : url.report(r.suite, id.kernel, r.version)}
                  />
                )
              })}
            </div>
          </div>
        )}
      </Section>
    </>
  )
}

// What a report's own words mean, said once beside the number they label.
const FAILURE_GLOSS: Record<(typeof FAILURES)[number], string> = {
  geometry: 'the shape itself is wrong',
  measurement: 'the shape is right, a measured value is off',
}
const KIND_GLOSS: Record<(typeof KINDS)[number], string> = {
  exact: 'one answer is right',
  sound: 'any valid answer within limits',
  rejection: 'input the kernel must refuse',
}

function Stat({
  label,
  gloss,
  value,
  sub,
}: {
  label: string
  gloss?: string
  value: string | number
  sub?: string
}) {
  return (
    <div className="rp-stat">
      <dt>
        <span className="bc-annot">{label}</span>
        {gloss && <span className="rp-stat__gloss">{gloss}</span>}
      </dt>
      <dd>
        <span className="rp-stat__value">{value}</span>
        {sub && <span className="rp-stat__sub">{sub}</span>}
      </dd>
    </div>
  )
}

function GradeRow({
  grade,
  count,
  total,
  href,
  onPick,
}: {
  grade: Grade
  count: number
  total: number
  href: string
  onPick: (e: MouseEvent) => void
}) {
  const share = total ? count / total : 0
  const body = (
    <>
      <GradeLabel grade={grade} />
      <span className="rp-bar" aria-hidden="true">
        {count > 0 && (
          <span
            className="rp-bar__fill"
            data-grade={grade}
            style={{ width: `max(1px, ${(share * 100).toFixed(2)}%)` }}
          />
        )}
      </span>
      <span className="rp-dist__n">{count}</span>
      <span className="rp-dist__pct">{fmt.pct(share)}</span>
    </>
  )
  if (!count)
    return (
      <div className="rp-dist__row" data-empty="">
        {body}
      </div>
    )
  return (
    <a
      className="rp-dist__row sc-focus"
      href={href}
      onClick={onPick}
      title={`show the ${count} ${grade} cases`}
    >
      {body}
    </a>
  )
}

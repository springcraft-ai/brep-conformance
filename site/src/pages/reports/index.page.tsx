import { fmt, PageHead, Table } from '@/components'
import { EmptyState } from '@/ds'
import { href, url } from '@/site/href'
import type { Props } from './index.data'
import type { ReportRow } from './list'
import './reports.css'

// Every published report, newest run first. A report is one suite version and every kernel run on
// it; its page keeps its leaderboard once a newer suite replaces it on the home page.
export default function Reports({ reports }: Props) {
  if (!reports.length)
    return (
      <div className="fn-page">
        <PageHead title="Reports" />
        <EmptyState
          title="no report published yet"
          height={320}
          action={<a href={href('/docs/#introduction-add-your-kernel')}>add your kernel</a>}
        />
      </div>
    )
  return (
    <div className="fn-page">
      <PageHead title="Reports" />
      <p className="fn-lede">
        Each report is one version of the suite, with every kernel run on it. The newest run is
        first.
      </p>
      <div className="rl-table">
        <Table<ReportRow>
          aria-label="reports"
          rowKey="suite"
          sortBy={null}
          rows={reports}
          columns={[
            {
              key: 'suite',
              label: 'suite',
              sortable: false,
              render: r => (
                <span className="rl-suite">
                  <a href={url.report(r.suite)}>{r.suite}</a>
                  {r.current && <span className="bc-annot rl-current">current</span>}
                </span>
              ),
            },
            {
              key: 'runs',
              label: 'run',
              sortable: false,
              render: r => (
                <span className="rl-nowrap">
                  {r.runs.length ? r.runs.map(fmt.mdy).join(' – ') : '–'}
                </span>
              ),
            },
            { key: 'kernels', label: 'kernels', align: 'right', numeric: true, sortable: false },
            {
              key: 'cases',
              label: 'cases',
              align: 'right',
              numeric: true,
              sortable: false,
              format: (n: number) => n.toLocaleString('en-US'),
            },
            {
              key: 'leader',
              label: 'first place',
              sortable: false,
              render: r =>
                r.leader ? (
                  <span className="rl-nowrap">
                    {r.leader.kernel} · {fmt.pct(r.leader.rate)} at 1e-7
                  </span>
                ) : (
                  '–'
                ),
            },
          ]}
        />
      </div>
    </div>
  )
}

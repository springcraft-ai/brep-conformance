import { fmt, GradeMark, PageHead, SearchBox } from '@/components'
import { takeSearch } from '@/components/search'
import { href, url } from '@/site/href'
import './operations.css'
import type { Props } from './index.data'

export const nav = { label: 'operations', order: 2 }

// Reference: every operation the suite specifies, and which kernels offer it, a column per kernel
// so a kernel reads down the page. What a kernel offers is a fact of one run of one version, so
// each column names its run and links to its report; how it does is the results page's, not this
// one's. The page ships no data; `enhance` lets its search box filter the rows already drawn.
export default function OperationsIndex({ total, suites, kernels, groups }: Props) {
  return (
    <>
      <div className="fn-page">
        <PageHead title="Operations" />
        <div className="fn-tools">
          <p className="fn-lede">
            Suite {suites.join(', ')} · {total} operations ·{' '}
            <a href={href('/docs/#constructors')}>how each is specified</a>
          </p>
          <SearchBox label="search operations" placeholder="e.g. fillet" />
        </div>
        <table className="fn-table">
          <thead>
            <tr>
              <th scope="col">
                <span className="fn-key">
                  <span>
                    <i className="fn-yes" /> offered
                  </span>
                  <span>
                    <GradeMark grade="unsupported" title="not offered" /> not offered
                  </span>
                  <span title="not offered; the suite sends the named operation in its place">
                    <span className="fn-standin">as revolve</span> stand-in
                  </span>
                </span>
              </th>
              {kernels.map(k => (
                <th key={k.kernel} scope="col" className="fn-kernel">
                  <a href={url.report(k.suite, k.kernel, k.version)}>
                    <span>{k.kernel}</span>
                    <span className="fn-run" role="tooltip">
                      {fmt.version(k.version)}
                      {k.date && <small>{fmt.mdy(k.date)} run</small>}
                    </span>
                  </a>
                  <span className="fn-pair">
                    <span className="fn-pair__n">
                      {k.offered}/{total}
                    </span>
                    <span className="fn-pair__of">offered</span>
                  </span>
                </th>
              ))}
            </tr>
          </thead>
          {groups.map(g => (
            <tbody key={g.group} data-group="">
              <tr className="fn-group">
                <th scope="rowgroup">
                  <span className="fn-cell">
                    <span className="fn-group__name">{g.group}</span>
                    {g.areas.length > 0 && (
                      <span className="bc-annot">
                        tested by{' '}
                        {g.areas.map((a, i) => (
                          <span key={a}>
                            {i > 0 && ' and '}
                            <a href={`${url.home()}#${a}`}>{a}</a>
                          </span>
                        ))}{' '}
                        tests
                      </span>
                    )}
                  </span>
                </th>
                {kernels.map((k, i) => (
                  <td key={k.kernel}>
                    <span className="fn-pair">
                      <span className="fn-pair__n">
                        {g.functions.filter(f => f.cells[i] === true).length}/{g.functions.length}
                      </span>
                    </span>
                  </td>
                ))}
              </tr>
              {g.functions.map(f => (
                <tr key={f.id} data-fn={`${f.id} ${g.group}`.toLowerCase()}>
                  <th scope="row">
                    <span className="fn-cell">
                      <a href={url.operation(f.id)}>{f.id}</a>
                      {f.signature && <code className="fn-sig bc-machine">{f.signature}</code>}
                    </span>
                  </th>
                  {f.cells.map((c, i) => {
                    const kernel = kernels[i]!.kernel
                    return (
                      <td key={kernel}>
                        {c === true ? (
                          <i className="fn-yes" role="img" aria-label={`${kernel} offers it`} />
                        ) : c === false ? (
                          <GradeMark grade="unsupported" title={`${kernel} does not offer it`} />
                        ) : (
                          <span
                            className="fn-standin"
                            title={`${kernel} does not offer it; the suite sends ${c} instead`}
                          >
                            as {c}
                          </span>
                        )}
                      </td>
                    )
                  })}
                </tr>
              ))}
            </tbody>
          ))}
        </table>
        <p className="bc-annot fn-none" id="fn-none" hidden>
          no operation matches
        </p>
      </div>
    </>
  )
}

export function enhance() {
  const rows = [...document.querySelectorAll<HTMLElement>('tr[data-fn]')]
  const groups = [...document.querySelectorAll<HTMLElement>('tbody[data-group]')]
  const none = document.getElementById('fn-none')
  const apply = (q: string) => {
    const words = q.trim().toLowerCase().split(/\s+/).filter(Boolean)
    for (const r of rows) r.hidden = !words.every(w => r.dataset.fn!.includes(w))
    for (const g of groups) g.hidden = !g.querySelector('tr[data-fn]:not([hidden])')
    const shown = rows.filter(r => !r.hidden).length
    if (none) none.hidden = shown > 0
  }
  takeSearch(apply)
}

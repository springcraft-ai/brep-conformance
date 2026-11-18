import { fmt, GradeMark, PageHead, SearchBox } from '@/components'
import { useUrlQuery } from '@/components/search'
import { href, url } from '@/site/href'
import './operations.css'
import type { Props } from './index.data'

export const nav = { label: 'operations', order: 2 }
export const hydrate = true

// Reference: every operation the suite specifies, and which kernels offer it, a column per kernel
// so a kernel reads down the page. What a kernel offers is a fact of one run of one version, so
// each column names its run and links to its report; how it does is the results page's, not this
// one's. Its search box narrows the rows to the operations and groups that hold every word.
export default function OperationsIndex({ total, suites, kernels, groups }: Props) {
  const [q, setQ] = useUrlQuery()
  const words = q.trim().toLowerCase().split(/\s+/).filter(Boolean)
  const shown = groups
    .map(g => ({
      ...g,
      rows: g.functions.filter(f => {
        const text = `${f.id} ${g.group}`.toLowerCase()
        return words.every(w => text.includes(w))
      }),
    }))
    .filter(g => g.rows.length)
  return (
    <>
      <div className="fn-page">
        <PageHead title="Operations" />
        <div className="fn-tools">
          <p className="fn-lede">
            Suite {suites.join(', ')} · {total} operations ·{' '}
            <a href={href('/docs/#constructors')}>how each is specified</a>
          </p>
          <SearchBox
            label="search operations"
            placeholder="e.g. fillet"
            value={q}
            onChange={setQ}
          />
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
                  <a href={url.kernel(k.suite, k.segment)}>
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
          {shown.map(g => (
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
              {g.rows.map(f => (
                <tr key={f.id} data-fn="">
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
        {!shown.length && <p className="bc-annot fn-none">no operation matches</p>}
      </div>
    </>
  )
}

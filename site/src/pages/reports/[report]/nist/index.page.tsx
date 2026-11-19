import { fmt, PageHead, Table, type Column } from '@/components'
import type { NistFile, NistRow } from '@/data/results'
import { url } from '@/site/href'
import type { Props } from './index.data'
import './nist.css'

export const hydrate = true

const measure = (x: number | null) =>
  x === null ? '–' : x.toLocaleString('en-US', { maximumSignificantDigits: 6 })

// A read that gave a solid agrees, differs from what the file states, or is read where the file
// states nothing to compare; a read that gave none shows its outcome.
function word(n: NistRow): { text: string; mark: string } {
  if (n.outcome) return { text: n.outcome, mark: 'nist-failed' }
  if (n.agrees === true) return { text: 'agrees', mark: 'nist-agrees' }
  if (n.agrees === false) return { text: 'differs', mark: 'nist-differs' }
  return { text: 'read', mark: 'nist-read' }
}

function Cell({ n }: { n: NistRow | undefined }) {
  if (!n) return <span className="nist-cell nist-none">–</span>
  const { text, mark } = word(n)
  const detail = n.outcome
    ? (n.error ?? n.outcome)
    : `volume ${measure(n.volume)} · area ${measure(n.area)}` +
      (n.valid === false ? ' · not valid' : '') +
      (n.ms === null ? '' : ` · ${fmt.ms(n.ms)}`)
  return (
    <span className={`nist-cell ${mark}`} title={detail}>
      {text}
      {n.valid === false && !n.outcome && <span className="nist-flag"> not valid</span>}
    </span>
  )
}

// NIST's STEP test files as each kernel of a report read them: a row per file, a column per
// kernel. Interop, which the score leaves out.
export default function Nist({ suite, kernels, files }: Props) {
  const columns: Column<NistFile>[] = [
    {
      key: 'file',
      label: 'file',
      sortable: false,
      render: f => {
        const cut = f.file.lastIndexOf('/') + 1
        return (
          <span className="nist-file">
            <span className="nist-folder">{f.file.slice(0, cut)}</span>
            {f.file.slice(cut)}
          </span>
        )
      },
    },
    {
      key: 'volume',
      label: 'stated volume',
      align: 'right',
      sortable: false,
      render: f => <span className="nist-num">{measure(f.statedVolume)}</span>,
    },
    {
      key: 'area',
      label: 'stated area',
      align: 'right',
      sortable: false,
      render: f => <span className="nist-num">{measure(f.statedArea)}</span>,
    },
    ...kernels.map(k => ({
      key: `k:${k.kernel}`,
      label: k.kernel,
      sortable: false,
      render: (f: NistFile) => <Cell n={f.kernels[k.kernel]} />,
    })),
  ]
  const read = (k: string) => files.filter(f => f.kernels[k] && !f.kernels[k]!.outcome).length
  return (
    <div className="fn-page nist">
      <PageHead
        trail={[{ label: `Report ${suite}`, href: url.report(suite) }]}
        title="NIST files"
      />
      <p className="nist-lede">
        NIST&rsquo;s STEP test files, read by each kernel and measured. This is interop and not part
        of the score. A file <em>agrees</em> when its volume and area are within 0.1% of those its
        exporter states in it, and is <em>read</em> where it states neither.
      </p>
      <ul className="nist-totals">
        {kernels.map(k => (
          <li key={k.kernel}>
            <a href={url.kernel(suite, k.segment)}>{k.kernel}</a> read {read(k.kernel)} of{' '}
            {files.length}
          </li>
        ))}
      </ul>
      <div className="nist-scroll">
        <Table<NistFile>
          aria-label="NIST files"
          rowKey="file"
          sortBy={null}
          rows={files}
          columns={columns}
        />
      </div>
    </div>
  )
}

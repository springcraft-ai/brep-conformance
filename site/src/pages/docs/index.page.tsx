import type { DocEntry, DocGroup } from '@/docs/source'
import { hydrateRoot } from 'react-dom/client'
import { PageHead } from '@/components'
import { prettyJson } from '@/docs/example'
import { Markdown, type DocLinks } from '@/pages/operations/Markdown'
import { href } from '@/site/href'
import { Contents, tocOf, type TocPart } from './Contents'
import type { Props } from './index.data'
import '../operations/operations.css'
import './docs.css'

export const nav = { label: 'docs', order: 3 }

const CONTENTS_ID = 'dx-contents'

// The docs on one page: contents on the left, definitions in the middle, and beside each function
// a real call and its reply.
export default function Docs({ parts, links }: Props) {
  return (
    <div className="fn-page dx-page">
      <PageHead title="Docs" />
      <div className="dx">
        <div id={CONTENTS_ID} className="dx-island">
          <Contents toc={tocOf(parts)} />
        </div>
        <div className="dx-main">
          {parts.map(p => (
            <div key={p.id}>
              <section className="dx-part" id={p.id} data-g={p.groups[0]?.id}>
                <h2>{p.title}</h2>
                {p.intro && (
                  <div className="bc-op-prose">
                    <Markdown source={p.intro} links={links[p.file]!} />
                  </div>
                )}
              </section>
              {p.groups.map(g => (
                <Group key={g.id} group={g} links={links[p.file]!} />
              ))}
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

function Group({ group: g, links }: { group: DocGroup; links: DocLinks }) {
  const wide = g.functions ? '' : ' dx-entry--wide'
  return (
    <>
      <section className={`dx-entry dx-entry--head${wide}`} id={g.id} data-g={g.id}>
        <div className="dx-doc bc-op-prose">
          <h3>
            <a href={`#${g.id}`}>{g.heading}</a>
          </h3>
          {g.body && <Markdown source={g.body} links={links} />}
        </div>
        {g.functions && <div className="dx-ex" />}
      </section>
      {g.entries.map(e => (
        <Entry key={e.id} entry={e} group={g} links={links} />
      ))}
    </>
  )
}

function Entry({
  entry: e,
  group: g,
  links,
}: {
  entry: DocEntry
  group: DocGroup
  links: DocLinks
}) {
  const wide = g.functions ? '' : ' dx-entry--wide'
  return (
    <section className={`dx-entry${wide}`} id={e.id} data-g={g.id}>
      <div className="dx-doc bc-op-prose">
        <h4 className={e.fn ? 'dx-fn' : 'dx-sub'}>
          <a href={`#${e.id}`}>{e.heading}</a>
        </h4>
        {e.body && <Markdown source={e.body} links={links} />}
        {e.fn && (
          <a
            className="dx-support"
            href={`${href('/operations/')}?q=${encodeURIComponent(e.heading)}`}
          >
            which kernels offer it
          </a>
        )}
      </div>
      {g.functions && (
        <div className="dx-ex">
          {e.example ? (
            <div className="dx-stick">
              <div className="dx-box dx-box--req">
                <div className="dx-cap">request</div>
                <pre tabIndex={0} aria-label={`${e.heading} request`}>
                  {prettyJson(e.example.request)}
                </pre>
              </div>
              <div className="dx-box dx-box--res">
                <div className="dx-cap">reply</div>
                <pre tabIndex={0} aria-label={`${e.heading} reply`}>
                  {prettyJson(e.example.reply)}
                </pre>
              </div>
              {e.example.note && (
                <div className="dx-note">
                  <Markdown source={e.example.note} links={links} />
                </div>
              )}
            </div>
          ) : (
            e.fn && <p className="dx-none">no example yet</p>
          )}
        </div>
      )}
    </section>
  )
}

export const clientProps = ({ parts }: Props) => ({ toc: tocOf(parts) })

// Hydrates the contents, and lets a long example open in place rather than scroll inside itself.
export function enhance({ toc }: { toc: TocPart[] }) {
  const contents = document.getElementById(CONTENTS_ID)
  if (contents) hydrateRoot(contents, <Contents toc={toc} />)

  document.querySelector('.dx')?.classList.add('dx--js')
  for (const pre of document.querySelectorAll<HTMLElement>('.dx-box pre')) {
    if (pre.scrollHeight <= pre.clientHeight + 4) continue
    pre.dataset.long = ''
    const lines = (pre.textContent ?? '').split('\n').length
    const more = document.createElement('button')
    more.type = 'button'
    more.className = 'dx-more sc-focus'
    const label = () =>
      (more.textContent = 'open' in pre.dataset ? 'show less' : `show all ${lines} lines`)
    more.addEventListener('click', () => {
      if ('open' in pre.dataset) delete pre.dataset.open
      else pre.dataset.open = ''
      label()
    })
    label()
    pre.after(more)
  }
}

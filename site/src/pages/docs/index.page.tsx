import type { DocEntry, DocGroup } from '@/docs/source'
import { takeSearch } from '@/components/search'
import { prettyJson } from '@/docs/example'
import { Markdown, type DocLinks } from '@/pages/operations/Markdown'
import { url } from '@/site/href'
import type { Props } from './index.data'
import '../operations/operations.css'
import './docs.css'

export const nav = { label: 'docs', order: 3 }

// The docs on one page: contents on the left, definitions in the middle, and beside each function
// a real call and its reply.
export default function Docs({ parts, links }: Props) {
  return (
    <div className="dx">
      <h1 className="bc-sr">Documentation</h1>
      <button type="button" id="dx-menu" className="dx-menu sc-focus" aria-expanded="false">
        contents
      </button>
      <nav className="dx-nav" aria-label="Contents">
        <ul>
          {parts.map(p => (
            <li key={p.id} className="dx-nav__part">
              <a href={`#${p.id}`}>{p.title}</a>
              <ul>
                {p.groups.map(g => (
                  <li key={g.id} className="dx-nav__g" data-g={g.id}>
                    <a href={`#${g.id}`}>{g.heading}</a>
                    {g.entries.length > 0 && (
                      <ul>
                        {g.entries.map(e => (
                          <li key={e.id}>
                            <a href={`#${e.id}`} className={e.fn ? 'dx-nav__fn' : undefined}>
                              {e.heading}
                            </a>
                          </li>
                        ))}
                      </ul>
                    )}
                  </li>
                ))}
              </ul>
            </li>
          ))}
        </ul>
      </nav>
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
          <a className="dx-support" href={url.operation(e.heading)}>
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

// Marks the section in view in the contents, opens its group there, and works the phone menu.
export function enhance() {
  const nav = document.querySelector<HTMLElement>('.dx-nav')
  const menu = document.getElementById('dx-menu')
  if (!nav || !menu) return
  // Without this script every group's list stays open; with it, only the one in view.
  nav.classList.add('dx-nav--js')
  const links = new Map(
    [...nav.querySelectorAll<HTMLAnchorElement>("a[href^='#']")].map(a => [
      a.getAttribute('href')!.slice(1),
      a,
    ]),
  )
  const groups = [...nav.querySelectorAll<HTMLElement>('.dx-nav__g')]
  const sections = [...document.querySelectorAll<HTMLElement>('.dx-part, .dx-entry')]
  let on: HTMLAnchorElement | undefined
  const chrome = document.querySelector<HTMLElement>('.bc-chrome')
  const spy = () => {
    // The section in view is the last one whose top has passed under the site's header.
    const line = (chrome?.getBoundingClientRect().bottom ?? 0) + 64
    let current = sections[0]
    for (const s of sections) {
      if (s.getBoundingClientRect().top <= line) current = s
      else break
    }
    if (!current) return
    const a = links.get(current.id)
    if (a === on) return
    on?.classList.remove('dx-on')
    on = a
    a?.classList.add('dx-on')
    groups.forEach(g => g.classList.toggle('dx-open', g.dataset.g === current.dataset.g))
    if (!finding) a?.scrollIntoView({ block: 'nearest' })
  }

  // Finding a section with the header's search: only the headings that match stay in the list,
  // with what they sit under, and Enter goes to the first.
  let finding = false
  const items = [...nav.querySelectorAll<HTMLLIElement>('li')]
  const narrow = (q: string) => {
    const words = q.trim().toLowerCase().split(/\s+/).filter(Boolean)
    finding = words.length > 0
    nav.classList.toggle('dx-nav--finding', finding)
    const hit = (li: HTMLLIElement) => {
      const text = li.querySelector('a')?.textContent?.toLowerCase() ?? ''
      return words.every(w => text.includes(w))
    }
    for (const li of items) li.hidden = finding && !hit(li)
    if (finding)
      for (const li of items.filter(hit))
        for (let up = li.parentElement?.closest('li'); up; up = up.parentElement?.closest('li'))
          up.hidden = false
  }
  takeSearch(narrow, () => {
    const first = items.find(li => !li.hidden && li.querySelector('ul') === null)
    first?.querySelector('a')?.click()
  })

  // A long example opens in place rather than scrolling inside itself.
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
  let queued = false
  window.addEventListener(
    'scroll',
    () => {
      if (queued) return
      queued = true
      requestAnimationFrame(() => {
        queued = false
        spy()
      })
    },
    { passive: true },
  )
  const setMenu = (open: boolean) => {
    document.body.classList.toggle('dx-menu-open', open)
    menu.setAttribute('aria-expanded', String(open))
  }
  menu.addEventListener('click', () => setMenu(!document.body.classList.contains('dx-menu-open')))
  nav.addEventListener('click', e => {
    if ((e.target as HTMLElement).closest('a')) setMenu(false)
  })
  spy()
}

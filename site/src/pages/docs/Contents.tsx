import { useEffect, useRef, useState } from 'react'
import { SearchBox } from '@/components'
import { useUrlQuery } from '@/components/search'

export interface TocEntry {
  id: string
  heading: string
  fn: boolean
}

export interface TocGroup {
  id: string
  heading: string
  entries: TocEntry[]
}

export interface TocPart {
  id: string
  title: string
  groups: TocGroup[]
}

// The contents' own data: the headings, without the sections under them.
export function tocOf(parts: TocPart[]): TocPart[] {
  return parts.map(p => ({
    id: p.id,
    title: p.title,
    groups: p.groups.map(g => ({
      id: g.id,
      heading: g.heading,
      entries: g.entries.map(e => ({ id: e.id, heading: e.heading, fn: e.fn })),
    })),
  }))
}

const wordsOf = (q: string) => q.trim().toLowerCase().split(/\s+/).filter(Boolean)

// The contents narrowed to the headings that hold every word, each with what it sits under.
export function narrow(toc: TocPart[], q: string): TocPart[] {
  const words = wordsOf(q)
  if (!words.length) return toc
  const hit = (text: string) => words.every(w => text.toLowerCase().includes(w))
  return toc
    .map(p => ({
      ...p,
      groups: p.groups
        .map(g => ({ ...g, entries: g.entries.filter(e => hit(e.heading)) }))
        .filter(g => hit(g.heading) || g.entries.length),
    }))
    .filter(p => hit(p.title) || p.groups.length)
}

// The first heading of a narrowed contents that has nothing under it: where Enter goes.
export function firstLeaf(toc: TocPart[]): string | null {
  for (const p of toc) {
    for (const g of p.groups) {
      if (!g.entries.length) return g.id
      return g.entries[0]!.id
    }
  }
  return null
}

// The docs' contents and its search box. Hydrated on its own, so the page ships its headings
// twice and not its sections. It marks the section in view, opens that section's group, and on a
// phone is the menu the "contents" button opens.
export function Contents({ toc }: { toc: TocPart[] }) {
  const [q, setQ] = useUrlQuery()
  const [live, setLive] = useState(false)
  const [on, setOn] = useState<{ id: string; group: string | null } | null>(null)
  const [menu, setMenu] = useState(false)
  const [maxHeight, setMaxHeight] = useState<number | undefined>(undefined)
  const root = useRef<HTMLDivElement>(null)
  const nav = useRef<HTMLElement>(null)
  const current = useRef<HTMLAnchorElement>(null)
  const finding = wordsOf(q).length > 0
  const shown = narrow(toc, q)

  useEffect(() => {
    setLive(true)
    const sections = [...document.querySelectorAll<HTMLElement>('.dx-part, .dx-entry')]
    const chrome = document.querySelector<HTMLElement>('.bc-chrome')
    const page = document.querySelector<HTMLElement>('.dx')
    const read = () => {
      const top = chrome?.getBoundingClientRect().bottom ?? 0
      // The section in view is the last one whose top has passed under the site's header.
      let section = sections[0]
      for (const s of sections) {
        if (s.getBoundingClientRect().top <= top + 64) section = s
        else break
      }
      if (section) {
        const next = { id: section.id, group: section.dataset.g ?? null }
        setOn(was => (was?.id === next.id ? was : next))
      }
      // At the foot of the page the end of the docs would push the pinned contents up and its
      // search box out of view: the contents get shorter instead.
      const pinned = nav.current && getComputedStyle(nav.current).position === 'sticky'
      const room = (page?.getBoundingClientRect().bottom ?? Infinity) - top
      setMaxHeight(pinned && room < window.innerHeight - top ? Math.max(room, 0) : undefined)
    }
    let queued = 0
    const later = () => {
      if (!queued)
        queued = requestAnimationFrame(() => {
          queued = 0
          read()
        })
    }
    window.addEventListener('scroll', later, { passive: true })
    window.addEventListener('resize', later)
    read()
    return () => {
      cancelAnimationFrame(queued)
      window.removeEventListener('scroll', later)
      window.removeEventListener('resize', later)
    }
  }, [])

  useEffect(() => {
    if (!finding) current.current?.scrollIntoView({ block: 'nearest' })
  }, [on, finding])

  useEffect(() => {
    if (!menu) return
    const outside = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) setMenu(false)
    }
    const escape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setMenu(false)
    }
    document.addEventListener('pointerdown', outside)
    document.addEventListener('keydown', escape)
    return () => {
      document.removeEventListener('pointerdown', outside)
      document.removeEventListener('keydown', escape)
    }
  }, [menu])

  const link = (id: string, text: string, className?: string) => {
    const here = on?.id === id
    return (
      <a
        href={`#${id}`}
        ref={here ? current : undefined}
        className={[className, here && 'dx-on'].filter(Boolean).join(' ') || undefined}
        onClick={() => setMenu(false)}
      >
        {text}
      </a>
    )
  }
  const navClass = ['dx-nav', live && 'dx-nav--js', finding && 'dx-nav--finding']
    .filter(Boolean)
    .join(' ')

  return (
    <div ref={root} className={`dx-contents${menu ? ' dx-menu-open' : ''}`}>
      <button
        type="button"
        className="dx-menu sc-focus"
        aria-expanded={menu}
        onClick={() => setMenu(!menu)}
      >
        contents
      </button>
      <nav ref={nav} className={navClass} aria-label="Contents" style={{ maxHeight }}>
        <SearchBox
          label="find a section"
          placeholder="e.g. tolerance"
          value={q}
          onChange={setQ}
          onEnter={() => {
            const id = firstLeaf(shown)
            if (!finding || !id) return
            window.location.hash = id
            setMenu(false)
          }}
        />
        {finding && !shown.length && <p className="bc-annot dx-nav__none">no section matches</p>}
        <ul>
          {shown.map(p => (
            <li key={p.id} className="dx-nav__part">
              {link(p.id, p.title)}
              <ul>
                {p.groups.map(g => (
                  <li key={g.id} className={`dx-nav__g${on?.group === g.id ? ' dx-open' : ''}`}>
                    {link(g.id, g.heading)}
                    {g.entries.length > 0 && (
                      <ul>
                        {g.entries.map(e => (
                          <li key={e.id}>
                            {link(e.id, e.heading, e.fn ? 'dx-nav__fn' : undefined)}
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
    </div>
  )
}

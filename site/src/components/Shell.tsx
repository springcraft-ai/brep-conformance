import type { ReactNode } from 'react'
import { Logo } from '@/ds'
import { href } from '@/site/href'
import { SUITE_URL } from '@/site/repo'
import { hasOwnSearch } from './search'

export interface NavItem {
  label: string
  order: number
  path: string
}

export interface ShellProps {
  path: string
  nav: NavItem[]
  suite: string
  children: ReactNode
}

// GitHub's mark, for the link to the suite's source.
const GH_MARK =
  'M8 0c4.42 0 8 3.58 8 8a8.013 8.013 0 0 1-5.45 7.59c-.4.08-.55-.17-.55-.38 0-.27.01-1.13.01-2.2 0-.75-.25-1.23-.54-1.48 1.78-.2 3.65-.88 3.65-3.95 0-.88-.31-1.59-.82-2.15.08-.2.36-1.02-.08-2.12 0 0-.67-.22-2.2.82-.64-.18-1.32-.27-2-.27-.68 0-1.36.09-2 .27-1.53-1.03-2.2-.82-2.2-.82-.44 1.1-.16 1.92-.08 2.12-.51.56-.82 1.28-.82 2.15 0 3.06 1.86 3.75 3.64 3.95-.23.2-.44.55-.51 1.07-.46.21-1.61.55-2.33-.66-.15-.24-.6-.83-1.23-.82-.67.01-.27.38.01.53.34.19.73.9.82 1.13.16.45.68 1.31 2.69.94 0 .67.01 1.3.01 1.49 0 .21-.15.45-.55.37A7.995 7.995 0 0 1 0 8c0-4.42 3.58-8 8-8Z'

function current(item: NavItem, path: string): boolean {
  return item.path === '/' ? path === '/' : path.startsWith(item.path)
}

// Rendered on the server around every page; it is never hydrated, so it holds no state.
export function Shell({ path, nav, suite, children }: ShellProps) {
  return (
    <>
      <a className="bc-skip" href="#content">
        Skip to content
      </a>
      <header className="bc-chrome">
        <a className="bc-chrome__id" href={href('/')}>
          <Logo lockup="mark" size={18} title="Springcraft" />
          <span>brep-conformance</span>
        </a>
        {!hasOwnSearch(path) && (
          <form className="bc-chrome__find" role="search" action={href('/')} method="get">
            <input
              name="q"
              type="search"
              className="sc-focus"
              placeholder="find a test, e.g. fillets or cylinders"
              aria-label="Find a test"
              autoComplete="off"
            />
          </form>
        )}
        <nav className="bc-chrome__nav" aria-label="site">
          {nav.map(item => (
            <a
              key={item.path}
              href={href(item.path)}
              aria-current={current(item, path) ? 'page' : undefined}
            >
              {item.label}
            </a>
          ))}
        </nav>
        <a className="bc-chrome__gh" href={SUITE_URL}>
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d={GH_MARK} />
          </svg>
          <span className="bc-chrome__gh-label">github</span>
        </a>
      </header>
      <main className="bc-main" id="content" tabIndex={-1}>
        {children}
      </main>
      <footer className="bc-foot">
        <span className="bc-annot">Maintained by Springcraft</span>
        <span className="bc-annot">
          suite {suite} · MIT · <a href={SUITE_URL}>source</a>
        </span>
      </footer>
    </>
  )
}

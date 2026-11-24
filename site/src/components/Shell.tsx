import type { ReactNode } from 'react'
import { Logo } from '@/ds'
import { GitHubMark } from './GitHubMark'
import { href } from '@/site/href'
import { SUITE_URL } from '@/site/repo'

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
          <GitHubMark />
          <span className="bc-chrome__gh-label">github</span>
        </a>
      </header>
      <main className="bc-main" id="content" tabIndex={-1}>
        {children}
      </main>
      <footer className="bc-foot">
        <span className="bc-annot">Maintained by Springcraft</span>
        <span className="bc-annot">
          suite {suite} · MIT / Apache-2.0 · <a href={SUITE_URL}>source</a>
        </span>
      </footer>
    </>
  )
}

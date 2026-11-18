import type { ReactNode } from 'react'

export function Annot({ children }: { children: ReactNode }) {
  return <span className="bc-annot">{children}</span>
}

// One page above this one in its breadcrumb.
export interface Crumb {
  label: string
  href: string
}

// The page's display title on the left, its registration labels pinned to the far corner, and
// above the title the pages it sits under.
export function PageHead({
  title,
  corner,
  trail,
}: {
  title: ReactNode
  corner?: ReactNode[]
  trail?: Crumb[]
}) {
  return (
    <div className={trail ? 'bc-head bc-head--trail' : 'bc-head'}>
      {trail && (
        <nav className="bc-crumbs" aria-label="Breadcrumb">
          <ol>
            {trail.map((c, i) => (
              <li key={i}>
                <a href={c.href}>{c.label}</a>
              </li>
            ))}
          </ol>
        </nav>
      )}
      <h1 className="bc-display">{title}</h1>
      {corner && (
        <div className="bc-head__corner">
          {corner.map((c, i) => (
            <Annot key={i}>{c}</Annot>
          ))}
        </div>
      )}
    </div>
  )
}

export function Section({
  label,
  aside,
  children,
}: {
  label: string
  aside?: ReactNode
  children: ReactNode
}) {
  return (
    <section className="bc-section">
      <div className="bc-section__label">
        <h2 className="bc-annot bc-section__title">{label}</h2>
        {aside && <Annot>{aside}</Annot>}
      </div>
      {children}
    </section>
  )
}

export function Stat({ value, label }: { value: ReactNode; label: string }) {
  return (
    <div className="bc-stat">
      <span className="bc-stat__value">{value}</span>
      <Annot>{label}</Annot>
    </div>
  )
}

// A region with nothing to show yet: hatched, labelled, never empty.
export function Hatched({ label, height }: { label: string; height?: number }) {
  return (
    <div
      className="bc-hatched sc-hatch sc-hatch"
      style={height ? { minHeight: height } : undefined}
    >
      <span className="bc-annot">{label}</span>
    </div>
  )
}

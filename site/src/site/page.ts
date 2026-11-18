import type { ComponentType } from 'react'
import type { Params } from './routes'

export type { Params }

// A page: <name>.page.tsx. Loaded on the server to render, and in the browser only to hydrate, so
// it must not import src/data/source.ts or anything else that reads files.
export interface PageModule<P = Record<string, unknown>> {
  default: ComponentType<P>
  // Ship the page's JS and props and hydrate it in the browser. Off by default: a page with no
  // interaction stays plain HTML and CSS.
  hydrate?: boolean
  // Run in the browser on the server-rendered page, for a page that needs a little behaviour but
  // not React throughout: it ships the page's JS, and of its props only what `clientProps` picks.
  // Ignored when the page hydrates.
  enhance?: (props: never) => void
  clientProps?: (props: P) => unknown
  // A static page that sets this joins the chrome's navigation.
  nav?: { label: string; order: number }
  // Title for a page with no data module.
  title?: string
}

export interface PageEntry<P = Record<string, unknown>> {
  params?: Params
  props: P
  title: string
  description?: string
  // The whole <title>, for a page that does not take the site's name after its own.
  fullTitle?: string
  // What a link to the page shows when shared, where that differs from its title and description.
  social?: { title: string; description: string }
}

// A page's data: <name>.data.ts beside <name>.page.tsx. Server only. Returns one entry per URL the
// page renders; a page without params returns one.
export interface DataModule<P = Record<string, unknown>> {
  entries(): PageEntry<P>[]
}

// An endpoint: <name>.<ext>.ts, rendering a non-HTML file such as an SVG badge. Server only.
export interface EndpointModule<P = Record<string, unknown>> {
  entries(): { params?: Params; props: P }[]
  render(props: P): string
}

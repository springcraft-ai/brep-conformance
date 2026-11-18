import '@/styles/site.css'
import { createElement } from 'react'
import { hydrateRoot } from 'react-dom/client'
import type { PageModule } from '@/site/page'

// Each page is its own chunk; only the one this document was rendered from is loaded.
const pages = import.meta.glob<PageModule>('./pages/**/*.page.tsx')

const boot = document.getElementById('page-data')
const root = document.getElementById('page')
const { module, props } = JSON.parse(boot?.textContent ?? '{}') as {
  module?: string
  props?: Record<string, unknown>
}
const load = module ? pages[module] : undefined
// A page's styles are in once its module has loaded; only then does the dev server show it.
void (load ? load() : Promise.resolve(undefined))
  .then(mod => {
    if (mod?.hydrate && root) hydrateRoot(root, createElement(mod.default, props ?? {}))
    else mod?.enhance?.(props as never)
  })
  .finally(() => {
    const hidden = document.getElementById('dev-hide')
    if (!hidden) return
    hidden.remove()
    // The browser went to the anchor while the page had no styles; go again now it has them.
    if (location.hash)
      document.getElementById(decodeURIComponent(location.hash.slice(1)))?.scrollIntoView()
  })

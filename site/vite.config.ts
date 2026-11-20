import { createReadStream, readdirSync, statSync } from 'node:fs'
import type { IncomingMessage, ServerResponse } from 'node:http'
import { join, resolve, sep } from 'node:path'
import react from '@vitejs/plugin-react'
import { defineConfig, type Plugin } from 'vite'
import { ASSET_BASE, LOCAL_ASSETS } from './src/data/results'

// With BREP_ASSETS_DIR, the dev and preview servers answer <base>local-assets/<path> with that
// folder's file, or send the request on to the CDN's <path>: a local run's files load before they
// are published, and the published reports' as before.
function localAssets(): Plugin {
  const dir = process.env.BREP_ASSETS_DIR ? resolve(process.env.BREP_ASSETS_DIR) : null
  const serve =
    (base: string) =>
    (req: IncomingMessage, res: ServerResponse, next: () => void): void => {
      const prefix = `${base}${LOCAL_ASSETS}/`
      const path = new URL(req.url ?? '/', 'http://x').pathname
      if (!dir || !path.startsWith(prefix)) return next()
      const rest = path.slice(prefix.length)
      const file = resolve(dir, decodeURIComponent(rest))
      if (file.startsWith(dir + sep) && statSync(file, { throwIfNoEntry: false })?.isFile())
        return void createReadStream(file).pipe(res)
      res.writeHead(302, { Location: `${ASSET_BASE}/${rest}` }).end()
    }
  return {
    name: 'local-assets',
    configureServer: server => void server.middlewares.use(serve(server.config.base)),
    configurePreviewServer: server => void server.middlewares.use(serve(server.config.base)),
  }
}

// Renders each request with the same server entry the static build uses.
function devPages(): Plugin {
  return {
    name: 'dev-pages',
    apply: 'serve',
    configureServer(server) {
      server.middlewares.use(async (req, res, next) => {
        // Vite's own modules (/@vite/client, /@react-refresh, /@fs/…) have no extension, so the
        // site's router would answer them with its 404 page and the client would never load.
        const path = new URL(req.originalUrl ?? '/', 'http://x').pathname
        if (path.slice(server.config.base.length - 1).startsWith('/@')) return next()
        try {
          const mod = await server.ssrLoadModule('/src/entry-server.tsx')
          const page = mod.render(req.originalUrl ?? '/')
          if (!page) return next()
          const html = page.type.startsWith('text/html')
          res.statusCode = page.status
          res.setHeader('Content-Type', page.type)
          res.end(html ? await server.transformIndexHtml(req.originalUrl!, page.body) : page.body)
        } catch (e) {
          server.ssrFixStacktrace(e as Error)
          next(e)
        }
      })
    },
  }
}

function pageModules(): string[] {
  return readdirSync(resolve(__dirname, 'src/pages'), { recursive: true, encoding: 'utf8' })
    .filter(f => f.endsWith('.page.tsx'))
    .map(f => join('src/pages', f))
    .sort()
}

export default defineConfig(({ isSsrBuild }) => ({
  base: process.env.SITE_BASE ?? '/',
  appType: 'mpa',
  plugins: [react(), localAssets(), devPages()],
  // src/data/kernels.ts imports the registry from the repository's root.
  server: { fs: { allow: [resolve(__dirname, '..')] } },
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
    },
  },
  build: {
    manifest: !isSsrBuild,
    emptyOutDir: true,
    // Each page is an input as well as a lazy import of the client entry, so its chunk keeps its
    // own name in the manifest when another chunk shares code with it.
    rollupOptions: isSsrBuild ? {} : { input: ['src/entry-client.tsx', ...pageModules()] },
  },
  test: {
    environment: 'node',
    env: { BREP_REPORTS_DIR: resolve(__dirname, 'src/fixtures/reports'), BREP_ANY_KERNEL: '1' },
  },
}))

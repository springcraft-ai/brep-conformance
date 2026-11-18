import { resolve } from 'node:path'
import react from '@vitejs/plugin-react'
import { defineConfig, type Plugin } from 'vite'

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

export default defineConfig(({ isSsrBuild }) => ({
  base: process.env.SITE_BASE ?? '/',
  appType: 'mpa',
  plugins: [react(), devPages()],
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
    rollupOptions: isSsrBuild ? {} : { input: 'src/entry-client.tsx' },
  },
  test: {
    environment: 'node',
    env: { BREP_REPORTS_DIR: resolve(__dirname, 'src/fixtures/reports'), BREP_ANY_KERNEL: '1' },
  },
}))

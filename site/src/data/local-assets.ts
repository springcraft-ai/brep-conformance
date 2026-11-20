// BREP_ASSETS_DIR: a local copy of the CDN's folders, <suite>/<kernel>/<version>/<key>, for runs
// not yet published. The dev and preview servers serve it under the site's base at this path, and a
// report whose own folder is there loads its files from it. A deployed build never sets it.
export const LOCAL_ASSETS_PATH = 'local-assets'

// The URL of one report's folder under that path: each part encoded, as a version may hold spaces.
export function localAssetUrl(
  base: string,
  suite: string,
  kernel: string,
  version: string,
): string {
  const parts = [suite, kernel, version].map(encodeURIComponent).join('/')
  return `${base.replace(/\/*$/, '/')}${LOCAL_ASSETS_PATH}/${parts}`
}

// Every internal link goes through href, so the site works under a Pages project path (SITE_BASE).
export function href(path: string): string {
  return import.meta.env.BASE_URL + path.replace(/^\//, '')
}

// Each param encoded as pathOf in routes.ts encodes it, so a link names the file prerender wrote.
export const segment = (s: string) => encodeURIComponent(s).replace(/%40/g, '@')
const e = segment

export const url = {
  home: () => href('/'),
  // A report: every kernel run on one suite version.
  report: (report: string) => href(`/reports/${e(report)}/`),
  // One kernel in a report; `kernel` is its segment, the name or name@version (kernelSegment).
  kernel: (report: string, kernel: string) => href(`/reports/${e(report)}/${e(kernel)}/`),
  // One test across a report's kernels, by its path, such as booleans/degenerate/equal-cylinders.
  compare: (report: string, test: string) =>
    href(`/reports/${e(report)}/compare/${test.split('/').map(e).join('/')}/`),
  operation: (operation: string) => href(`/operations/${e(operation)}/`),
  badge: (kernel: string) => href(`/badges/${e(kernel)}.svg`),
}

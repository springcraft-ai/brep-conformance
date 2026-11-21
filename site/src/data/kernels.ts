// What the site states about each kernel beyond its report, from the registry the runner reads
// (kernels.json at the repository's root): its license, where its source is, and its adapter. Pure:
// a hydrated page may import it.

import registry from '../../../kernels.json'

export interface KernelInfo {
  kernel: string
  // The release an official run must report.
  version: string
  // An SPDX expression where the kernel has one.
  license: string
  source: string | null
  // The suite's maintainer makes the kernel, which a ranking must say.
  maintainers: boolean
  adapter: { repo: string; commit: string; license: string }
  // The kernel's own maintainers who review its adapter.
  reviewers: string[]
}

// A kernel's name and its version each name a folder of its files on the CDN and a page here:
// characters a URL never encodes, after a letter or digit. suite/src/registry.rs refuses the same.
const SEGMENT = /^[A-Za-z0-9][A-Za-z0-9._-]*$/

// Why an entry's name or version cannot name a folder; null when both can.
export function badSegment(k: { kernel: string; version: string }): string | null {
  for (const [field, value] of [
    ['kernel', k.kernel],
    ['version', k.version],
  ] as const) {
    if (!SEGMENT.test(value))
      return `${field} \`${value}\` is not letters, digits, ., _ and - after a letter or digit`
  }
  return null
}

registry.kernels.forEach((k, i) => {
  const bad = badSegment(k)
  if (bad) throw new Error(`kernels.json: kernels[${i}]: ${bad}`)
})

export const KERNELS: KernelInfo[] = registry.kernels.map(k => ({
  kernel: k.kernel,
  version: k.version,
  license: k.license,
  source: k.source,
  maintainers: k.maintained_by_suite,
  adapter: { repo: k.adapter.repo, commit: k.adapter.commit, license: k.adapter.license },
  reviewers: k.reviewers,
}))

export function kernelInfo(kernel: string): KernelInfo | null {
  return KERNELS.find(k => k.kernel === kernel) ?? null
}

// The adapter's repository's own name, as a reader knows it.
export function repoName(repo: string): string {
  return repo.replace(/\/+$/, '').split('/').pop() ?? repo
}

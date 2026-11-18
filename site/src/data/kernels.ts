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

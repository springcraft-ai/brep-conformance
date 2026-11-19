import { kernelSegment, nistIndex, type NistFile } from '@/data/results'
import { site } from '@/data/source'
import type { PageEntry } from '@/site/page'

export interface Props {
  suite: string
  // The kernels whose runs read NIST's files, each with its page in the report: kernelSegment.
  kernels: { kernel: string; segment: string }[]
  files: NistFile[]
}

// A page for each report whose runs read any of NIST's files.
export function entries(): PageEntry<Props>[] {
  return site().suites.flatMap(v => {
    const { kernels, files } = nistIndex(v.latest)
    if (files.length === 0) return []
    const segment = (k: string) => {
      const r = v.latest.find(l => l.identity.kernel === k)!
      return kernelSegment(v.reports, r)
    }
    return [
      {
        params: { report: v.suite },
        props: {
          suite: v.suite,
          kernels: kernels.map(k => ({ kernel: k, segment: segment(k) })),
          files,
        },
        title: `NIST files, report ${v.suite}`,
        description:
          `NIST's STEP test files as each kernel on suite ${v.suite} read them: interop, ` +
          'not part of the score.',
      },
    ]
  })
}

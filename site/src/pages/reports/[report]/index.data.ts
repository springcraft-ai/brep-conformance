import { site } from '@/data/source'
import type { PageEntry } from '@/site/page'
import { propsOf, type Props } from '../../index.data'

export function entries(): PageEntry<Props>[] {
  return site().suites.map(v => ({
    params: { report: v.suite },
    props: propsOf(v, false),
    title: `Report ${v.suite}`,
    description: `Every kernel run on suite ${v.suite} of the B-rep conformance suite.`,
  }))
}

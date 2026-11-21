import { site } from '@/data/source'
import type { PageEntry } from '@/site/page'
import { reportRows, type ReportRow } from './list'

export interface Props {
  reports: ReportRow[]
}

export function entries(): PageEntry<Props>[] {
  const s = site()
  return [
    {
      props: { reports: reportRows(s.suites, s.suite) },
      title: 'reports',
      description:
        'Every published report of the B-rep conformance suite, newest first: one per suite ' +
        'version, each with every kernel run on it.',
    },
  ]
}

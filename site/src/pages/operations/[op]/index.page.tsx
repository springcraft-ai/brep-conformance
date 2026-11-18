import { PageHead, Section } from '@/components'
import { Markdown } from '../Markdown'
import '../operations.css'
import type { Props } from './index.data'

// An operation's specification: what it takes, what it makes, and what its group has in common. It
// is reference, so it says nothing of any kernel: which kernels offer an operation is a fact of a
// run, and the operations page gives it with the run's date.
export default function OperationPage({ op, links }: Props) {
  const { doc } = op
  return (
    <div className="bc-op-page">
      <PageHead title={op.id} corner={[doc.group]} />

      <Section label="definition" aside={`api.md § ${doc.heading}`}>
        <div className="bc-op-prose">
          <Markdown source={doc.body} links={links} />
        </div>
      </Section>

      {doc.groupIntro && (
        <Section label={`common to ${doc.group.toLowerCase()}`}>
          <div className="bc-op-prose bc-op-prose--quiet">
            <Markdown source={doc.groupIntro} links={links} />
          </div>
        </Section>
      )}
    </div>
  )
}

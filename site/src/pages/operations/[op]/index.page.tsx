import { PageHead, Section } from '@/components'
import { href } from '@/site/href'
import { Markdown } from '../Markdown'
import '../operations.css'
import type { Props } from './index.data'

// An operation's specification: what it takes, what it makes, and what its group has in common. It
// is reference, so it says nothing of any kernel: which kernels offer an operation is a fact of a
// run, and the operations page gives it with the run's date. Its head is the operations page's, so
// going between the two moves nothing above the content.
export default function OperationPage({ op, links }: Props) {
  const { doc } = op
  return (
    <div className="fn-page bc-op-page">
      <PageHead title={op.id} />
      <div className="fn-tools">
        <p className="fn-lede">
          {doc.group} · api.md § {doc.heading} · <a href={href('/operations/')}>all operations</a>
        </p>
      </div>

      <Section label="definition">
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

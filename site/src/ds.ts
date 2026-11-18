// The Springcraft design system's components, from the build scripts/fetch-ds.mjs fetches.
import build from '../.ds/sc.mjs'

export const {
  Checkbox,
  DataTable,
  EmptyState,
  Logo,
  OutlineButton,
  OutlineField,
  Segmented,
  Select,
  SpecTable,
} = build

export type DataColumn<R> = SC.DataColumn<R>
export type DataTableProps<R> = SC.DataTableProps<R>

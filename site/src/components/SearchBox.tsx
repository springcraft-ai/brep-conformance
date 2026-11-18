import { SEARCH_ID } from './search'

// A page's search box, placed by what it narrows. Without JS it submits ?q= to the page it is on.
export function SearchBox({ label, placeholder }: { label: string; placeholder: string }) {
  return (
    <form className="bc-search" role="search" method="get">
      <label htmlFor={SEARCH_ID}>{label}</label>
      <input
        id={SEARCH_ID}
        name="q"
        type="search"
        className="sc-focus"
        placeholder={placeholder}
        autoComplete="off"
      />
    </form>
  )
}

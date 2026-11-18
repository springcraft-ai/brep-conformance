import { SEARCH_ID } from './search'

// A page's search box, placed by what it narrows. A React page passes `value` and `onChange`; a
// page that searches without React leaves them out and reads the box with `takeSearch`.
export function SearchBox({
  label,
  placeholder,
  value,
  onChange,
}: {
  label: string
  placeholder: string
  value?: string
  onChange?: (q: string) => void
}) {
  return (
    <div className="bc-search" role="search">
      <label htmlFor={SEARCH_ID}>{label}</label>
      <input
        id={SEARCH_ID}
        type="search"
        className="sc-focus"
        placeholder={placeholder}
        autoComplete="off"
        {...(onChange && { value: value ?? '', onChange: e => onChange(e.target.value) })}
      />
    </div>
  )
}

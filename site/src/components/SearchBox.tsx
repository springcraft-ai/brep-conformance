// A page's search box, placed by what it narrows. The page holds the query; Enter runs `onEnter`.
export function SearchBox({
  label,
  placeholder,
  value,
  onChange,
  onEnter,
}: {
  label: string
  placeholder: string
  value: string
  onChange: (q: string) => void
  onEnter?: () => void
}) {
  return (
    <label className="bc-search" role="search">
      <span>{label}</span>
      <input
        type="search"
        className="sc-focus"
        placeholder={placeholder}
        autoComplete="off"
        value={value}
        onChange={e => onChange(e.target.value)}
        onKeyDown={e => {
          if (e.key === 'Enter') onEnter?.()
        }}
      />
    </label>
  )
}

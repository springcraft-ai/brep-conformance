// The query a page was opened with, for its search box to start from.
export function queryFromUrl(): string {
  return new URLSearchParams(window.location.search).get('q') ?? ''
}

// The name a saved file takes: the last part of its address.
export function fileName(url: string): string {
  return url.slice(url.lastIndexOf('/') + 1)
}

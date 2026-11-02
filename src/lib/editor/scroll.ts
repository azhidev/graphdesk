/** Keep focus restoration from moving a document whose editor is already visible. */
export function needsReveal(target: Pick<DOMRect, 'top' | 'bottom'>, viewport: Pick<DOMRect, 'top' | 'bottom'>): boolean {
  return target.top < viewport.top || target.bottom > viewport.bottom;
}

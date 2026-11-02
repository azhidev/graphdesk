/** Keep focus restoration from moving a document whose editor is already visible. */
export function needsReveal(target: Pick<DOMRect, 'top' | 'bottom'>, viewport: Pick<DOMRect, 'top' | 'bottom'>): boolean {
  // A tall textarea cannot fit wholly in the viewport. If any of it is
  // visible, aligning its far edge would create a large, unhelpful jump.
  if (target.bottom - target.top >= viewport.bottom - viewport.top) {
    return target.bottom <= viewport.top || target.top >= viewport.bottom;
  }
  return target.top < viewport.top || target.bottom > viewport.bottom;
}

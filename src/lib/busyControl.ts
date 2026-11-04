/** Transient navigation lock, distinct from a permanently unavailable action.
 * Keep shell controls focusable/visually stable while their handlers are blocked.
 * Capture also covers native Enter/Space clicks before Svelte's delegated handler. */
export function busyControl(node: HTMLButtonElement, busy: boolean) {
  let blocked = busy;
  function update(value: boolean) {
    blocked = value;
    if (blocked) node.setAttribute('aria-disabled', 'true');
    else node.removeAttribute('aria-disabled'); // native disabled still describes intrinsic unavailability
  }
  function click(event: Event) {
    if (!blocked) return;
    event.preventDefault(); event.stopImmediatePropagation();
  }
  node.addEventListener('click', click, true);
  update(busy);
  return { update, destroy() { node.removeEventListener('click', click, true); } };
}

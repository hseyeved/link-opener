/** Focuses (and for text inputs, selects) the element when it mounts. */
export function focus(node: HTMLElement, select = true) {
  node.focus();
  if (select && node instanceof HTMLInputElement) node.select();
}

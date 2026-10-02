// Drag-and-drop state. The dragged item is kept here rather than read from `dataTransfer`,
// which browsers hide during `dragover`.
export type DragItem = { kind: "bookmark"; id: number } | { kind: "folder"; id: number };

/** Where a drop lands relative to the row under the pointer. */
export type DropZone = "before" | "after" | "into";

class Dnd {
  item = $state<DragItem | null>(null);
}

export const dnd = new Dnd();

/** Fraction (0–1) of the way down `e.currentTarget` the pointer is. */
export function pointerFraction(e: DragEvent): number {
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
  return rect.height ? (e.clientY - rect.top) / rect.height : 0.5;
}

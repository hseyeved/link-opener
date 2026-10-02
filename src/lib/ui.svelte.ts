// Which dialog is open, so any component can open one. The page renders them.
import * as api from "./api";
import { app, errorMessage } from "./state.svelte";
import type { Bookmark, Folder, ResolvedTarget } from "./types";

export type MoveItem = { kind: "bookmark"; bookmark: Bookmark } | { kind: "folder"; folder: Folder };

export interface Picking {
  bookmark: Bookmark;
  /** The bookmark's default browser, preselected in the picker. */
  resolved: ResolvedTarget | null;
  error: string | null;
}

class Ui {
  /** `{ bookmark: null }` = adding. */
  editing = $state<{ bookmark: Bookmark | null } | null>(null);
  picking = $state<Picking | null>(null);
  searching = $state(false);
  moving = $state<MoveItem | null>(null);
  /** Folder whose default browser is being edited. */
  folderTarget = $state<Folder | null>(null);
  settingsOpen = $state(false);
  browsersOpen = $state(false);

  get dialogOpen(): boolean {
    return !!(this.editing || this.picking || this.searching || this.moving || this.folderTarget || this.settingsOpen || this.browsersOpen);
  }

  closeAll() {
    this.editing = null;
    this.picking = null;
    this.searching = false;
    this.moving = null;
    this.folderTarget = null;
    this.settingsOpen = false;
    this.browsersOpen = false;
  }

  /** Opens with the bookmark's default browser if it has one (and opening directly is on),
   * else shows the picker. `forcePicker` (Shift) always shows the picker. */
  async open(bookmark: Bookmark, forcePicker = false) {
    let resolved: ResolvedTarget | null = null;
    try {
      resolved = await api.resolveTarget(bookmark.id);
    } catch (e) {
      this.picking = { bookmark, resolved: null, error: errorMessage(e) };
      return;
    }
    if (resolved && app.openDirect && !forcePicker) {
      try {
        await api.openUrl(bookmark.id, resolved.target);
        return;
      } catch (e) {
        // E.g. the default browser was uninstalled: let the user pick another.
        this.picking = { bookmark, resolved, error: errorMessage(e) };
        return;
      }
    }
    this.picking = { bookmark, resolved, error: null };
  }
}

export const ui = new Ui();

import { convertFileSrc } from "@tauri-apps/api/core";
import * as api from "./api";
import type { Bookmark, Browser, Folder, LaunchTarget } from "./types";

export interface FolderNode extends Folder {
  children: FolderNode[];
}

/** A folder with its depth, in tree (depth-first) order. */
export interface FlatFolder {
  folder: Folder;
  depth: number;
}

function buildTree(folders: Folder[]): FolderNode[] {
  const nodes = new Map<number, FolderNode>();
  for (const f of folders) nodes.set(f.id, { ...f, children: [] });
  const roots: FolderNode[] = [];
  // `folders` arrives ordered by parent then position, so children stay in order.
  for (const node of nodes.values()) {
    const parent = node.parentId === null ? undefined : nodes.get(node.parentId);
    (parent ? parent.children : roots).push(node);
  }
  return roots;
}

function flatten(nodes: FolderNode[], depth = 0, out: FlatFolder[] = []): FlatFolder[] {
  for (const node of nodes) {
    out.push({ folder: node, depth });
    flatten(node.children, depth + 1, out);
  }
  return out;
}

function message(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

const UNFILED_NAME_KEY = "unfiled_name";
const OPEN_DIRECT_KEY = "open_direct";
export const DEFAULT_UNFILED_NAME = "Unfiled";

class AppState {
  folders = $state<Folder[]>([]);
  /** `null` = Unfiled. */
  selectedFolderId = $state<number | null>(null);
  bookmarks = $state<Bookmark[]>([]);
  error = $state<string | null>(null);
  /** `null` until first loaded. */
  browsers = $state<Browser[] | null>(null);
  /** Display name of the root (folder_id = null). Renamable; stored as a setting. */
  unfiledName = $state(DEFAULT_UNFILED_NAME);

  tree = $derived(buildTree(this.folders));
  flatFolders = $derived(flatten(this.tree));
  selectedFolder = $derived(this.folders.find((f) => f.id === this.selectedFolderId) ?? null);
  #foldersById = $derived(new Map(this.folders.map((f) => [f.id, f])));

  /** Absolute path of the favicons dir; empty until loaded. */
  faviconDir = $state("");

  /** Open a bookmark's default browser directly, without showing the picker. */
  openDirect = $state(true);

  /** Image URL for a stored favicon file name. */
  faviconSrc(name: string | null): string | null {
    if (!name || !this.faviconDir) return null;
    const sep = this.faviconDir.includes("\\") ? "\\" : "/";
    return convertFileSrc(this.faviconDir + sep + name);
  }

  /** The folder and its ancestors, nearest first. */
  ancestors(folderId: number | null): Folder[] {
    const chain: Folder[] = [];
    // The depth bound guards against a (corrupt) parent cycle.
    for (let id = folderId; id !== null && chain.length < 1000; ) {
      const folder = this.#foldersById.get(id);
      if (!folder) break;
      chain.push(folder);
      id = folder.parentId;
    }
    return chain;
  }

  /** "Parent / Child" for a folder id, or the Unfiled name for `null`. */
  folderPath(folderId: number | null): string {
    const names = this.ancestors(folderId).map((f) => f.name).reverse();
    return names.length ? names.join(" / ") : this.unfiledName;
  }

  /** Whether `folderId` is `ancestorId` or inside it. */
  isWithin(folderId: number | null, ancestorId: number): boolean {
    return this.ancestors(folderId).some((f) => f.id === ancestorId);
  }

  /** The default browser a bookmark in `folderId` inherits: the nearest folder's, if any. */
  inheritedTarget(folderId: number | null): { target: LaunchTarget; folder: Folder } | null {
    const folder = this.ancestors(folderId).find((f) => f.defaultTarget);
    return folder?.defaultTarget ? { target: folder.defaultTarget, folder } : null;
  }

  async renameUnfiled(name: string) {
    await api.setSetting(UNFILED_NAME_KEY, name);
    this.unfiledName = name;
  }

  async setOpenDirect(value: boolean) {
    await api.setSetting(OPEN_DIRECT_KEY, String(value));
    this.openDirect = value;
  }

  /** After moves or deletes that can touch both the tree and the list. */
  async reloadAll() {
    await this.reloadFolders();
    await this.reloadBookmarks();
  }

  async reloadFolders() {
    this.folders = await api.listFolders();
    if (this.selectedFolderId !== null && !this.folders.some((f) => f.id === this.selectedFolderId)) {
      await this.select(null);
    }
  }

  async reloadBookmarks() {
    const folderId = this.selectedFolderId;
    const bookmarks = await api.listBookmarks(folderId);
    // Ignore a stale response if the selection changed meanwhile.
    if (folderId === this.selectedFolderId) this.bookmarks = bookmarks;
  }

  async loadBrowsers(refresh = false) {
    this.browsers = await api.listBrowsers(refresh);
  }

  async select(folderId: number | null) {
    this.selectedFolderId = folderId;
    await this.reloadBookmarks();
  }

  /** Runs an action and reports a failure in `error` instead of throwing. Returns success. */
  async run(action: () => Promise<unknown>): Promise<boolean> {
    try {
      await action();
      this.error = null;
      return true;
    } catch (e) {
      this.error = message(e);
      return false;
    }
  }

  async init() {
    await this.run(async () => {
      this.unfiledName = (await api.getSetting(UNFILED_NAME_KEY)) || DEFAULT_UNFILED_NAME;
      this.openDirect = (await api.getSetting(OPEN_DIRECT_KEY)) !== "false";
      this.faviconDir = await api.faviconDir();
      await this.reloadFolders();
      await this.reloadBookmarks();
    });
  }
}

export const app = new AppState();
export { message as errorMessage };

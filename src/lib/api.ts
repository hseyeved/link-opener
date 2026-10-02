// The only module that calls `invoke`. Errors reject with the backend's message string.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import type {
  Bookmark,
  BookmarkPatch,
  Browser,
  BrowserPrefs,
  DesktopSettings,
  ExportSummary,
  ImportMode,
  ImportPreview,
  ImportSummary,
  Folder,
  LaunchTarget,
  NewBookmark,
  PageMeta,
  ResolvedTarget,
  Tag,
} from "./types";

export const listFolders = () => invoke<Folder[]>("list_folders");

export const createFolder = (parentId: number | null, name: string) =>
  invoke<Folder>("create_folder", { parentId, name });

export const renameFolder = (id: number, name: string) =>
  invoke<Folder>("rename_folder", { id, name });

export const deleteFolder = (id: number) => invoke<void>("delete_folder", { id });

export const listBookmarks = (folderId: number | null) =>
  invoke<Bookmark[]>("list_bookmarks", { folderId });

export const createBookmark = (bookmark: NewBookmark) =>
  invoke<Bookmark>("create_bookmark", { bookmark });

export const updateBookmark = (id: number, patch: BookmarkPatch) =>
  invoke<Bookmark>("update_bookmark", { id, patch });

export const deleteBookmark = (id: number) => invoke<void>("delete_bookmark", { id });

/** Moves into `folderId` (`null` = Unfiled) at `index` among the new siblings (`null` = last). */
export const moveBookmark = (id: number, folderId: number | null, index: number | null = null) =>
  invoke<Bookmark>("move_bookmark", { id, folderId, index });

/** Moves under `parentId` (`null` = top level) at `index` among the new siblings (`null` = last). */
export const moveFolder = (id: number, parentId: number | null, index: number | null = null) =>
  invoke<Folder>("move_folder", { id, parentId, index });

export const setFolderTarget = (id: number, target: LaunchTarget | null) =>
  invoke<Folder>("set_folder_target", { id, target });

/** The bookmark's own default browser, else its nearest folder's, else `null`. */
export const resolveTarget = (bookmarkId: number) =>
  invoke<ResolvedTarget | null>("resolve_target", { bookmarkId });

export const listTags = () => invoke<Tag[]>("list_tags");

/** Title and favicon for a URL; can take several seconds. */
export const fetchMetadata = (url: string) => invoke<PageMeta>("fetch_metadata", { url });

/** Fetches a saved bookmark's favicon (and title, if it has none). */
export const refreshMetadata = (bookmarkId: number) => invoke<Bookmark>("refresh_metadata", { bookmarkId });

/** Fetches icons for all web bookmarks that have none; returns how many were found. */
export const fetchMissingFavicons = () => invoke<number>("fetch_missing_favicons");

export const faviconDir = () => invoke<string>("favicon_dir");

/** Detected browsers, cached by the backend until `refresh` is true. */
export const listBrowsers = (refresh = false) => invoke<Browser[]>("list_browsers", { refresh });

export const getBrowserPrefs = () => invoke<BrowserPrefs>("get_browser_prefs");

/** Saves browser prefs; returns the updated browser list. */
export const setBrowserPrefs = (prefs: BrowserPrefs) => invoke<Browser[]>("set_browser_prefs", { prefs });

export const openUrl = (bookmarkId: number, target: LaunchTarget) =>
  invoke<void>("open_url", { bookmarkId, target });

/** Bookmarks matching every term (substring of title, URL, notes or tags), best first.
 * An empty query returns recently opened bookmarks. */
export const search = (query: string, limit?: number) => invoke<Bookmark[]>("search", { query, limit });

export const getSetting = (key: string) => invoke<string | null>("get_setting", { key });

export const setSetting = (key: string, value: string) => invoke<void>("set_setting", { key, value });

/** The last browser/profile/private choice, saved by `openUrl`. */
export async function getLastTarget(): Promise<LaunchTarget | null> {
  const json = await getSetting("last_target");
  if (!json) return null;
  try {
    return JSON.parse(json) as LaunchTarget;
  } catch {
    return null;
  }
}

export const getDesktopSettings = () => invoke<DesktopSettings>("get_desktop_settings");

/** Registers and saves the global shortcut ("" = none); rejects if it can't be registered. */
export const setGlobalShortcut = (shortcut: string) =>
  invoke<DesktopSettings>("set_global_shortcut", { shortcut });

export const setCloseToTray = (enabled: boolean) => invoke<void>("set_close_to_tray", { enabled });

export const setAutostart = (enabled: boolean) => invoke<void>("set_autostart", { enabled });

/** The tray menu, global shortcut or a second launch asking to open a view. */
export const onOpenView = (handler: (view: "search" | "settings") => void): Promise<UnlistenFn> =>
  listen<"search" | "settings">("open-view", (e) => handler(e.payload));

/** Native "save as" dialog; `null` if cancelled. */
export const pickSavePath = (defaultPath: string, filter: { name: string; extensions: string[] }) =>
  saveDialog({ defaultPath, filters: [filter] });

/** Native "open" dialog for a backup or browser bookmarks file; `null` if cancelled. */
export async function pickImportPath(): Promise<string | null> {
  const path = await openDialog({
    multiple: false,
    directory: false,
    filters: [
      { name: "Backups and bookmark files", extensions: ["json", "html", "htm"] },
      { name: "All files", extensions: ["*"] },
    ],
  });
  return typeof path === "string" ? path : null;
}

/** Full backup: library, favicons and library settings, as JSON. */
export const exportBackup = (path: string) => invoke<ExportSummary>("export_backup", { path });

/** The library as a browser bookmarks file (HTML). */
export const exportHtml = (path: string) => invoke<ExportSummary>("export_html", { path });

export const inspectImport = (path: string) => invoke<ImportPreview>("inspect_import", { path });

/** Browser files always merge, into a new top-level folder named `folderName`. */
export const importFile = (path: string, mode: ImportMode, skipDuplicates: boolean, folderName: string | null) =>
  invoke<ImportSummary>("import_file", { path, mode, skipDuplicates, folderName });

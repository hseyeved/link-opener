// Mirrors the Rust models in src-tauri/src/db (serde camelCase).

export interface Folder {
  id: number;
  parentId: number | null;
  name: string;
  position: number;
  /** Browser for bookmarks in this folder and its subfolders, unless they set their own. */
  defaultTarget: LaunchTarget | null;
}

export interface Bookmark {
  id: number;
  /** `null` = Unfiled. */
  folderId: number | null;
  title: string;
  url: string;
  notes: string;
  favicon: string | null;
  position: number;
  /** Browser to open with instead of asking; overrides the folders' defaults. */
  defaultTarget: LaunchTarget | null;
  /** Unix milliseconds. */
  createdAt: number;
  updatedAt: number;
  lastOpenedAt: number | null;
  openCount: number;
  /** Sorted case-insensitively. */
  tags: string[];
}

export interface NewBookmark {
  folderId: number | null;
  title: string;
  url: string;
  notes: string;
  tags?: string[];
  defaultTarget?: LaunchTarget | null;
  /** File name from `fetchMetadata`. */
  favicon?: string | null;
}

/** Omitted fields are unchanged. `folderId: null` moves the bookmark to Unfiled;
 * `defaultTarget: null` clears its default browser. */
export interface BookmarkPatch {
  title?: string;
  url?: string;
  notes?: string;
  folderId?: number | null;
  tags?: string[];
  defaultTarget?: LaunchTarget | null;
  favicon?: string | null;
}

export type BrowserKind = "chromium" | "firefox" | "safari" | "other";

export interface Profile {
  /** Chromium: profile directory ("Profile 1"). Firefox: profile name. */
  id: string;
  /** The user's label if set, else `defaultName`. */
  name: string;
  email: string | null;
  defaultName: string;
  /** Left out of the picker. */
  hidden: boolean;
}

export interface Browser {
  id: string;
  /** The user's label if set, else `defaultName`. */
  name: string;
  kind: BrowserKind;
  profiles: Profile[];
  supportsPrivate: boolean;
  defaultName: string;
  /** Left out of the picker; saved defaults can still open it. */
  hidden: boolean;
  /** Added by the user rather than detected. */
  custom: boolean;
  /** The command, for display. */
  path: string;
}

/** Browser list preferences. Keys: "<browser id>" or "<browser id>/<profile id>". */
export interface BrowserPrefs {
  /** Browser ids, first to last; unlisted browsers follow by name. */
  order: string[];
  hidden: string[];
  labels: Record<string, string>;
  custom: CustomBrowser[];
}

export interface CustomBrowser {
  /** "" for a new one; assigned on save. */
  id: string;
  name: string;
  path: string;
  kind: BrowserKind;
}

export interface LaunchTarget {
  browserId: string;
  profileId: string | null;
  private: boolean;
}

export interface Tag {
  id: number;
  name: string;
  /** Number of bookmarks with this tag. */
  count: number;
}

/** A bookmark's effective default browser. */
export interface ResolvedTarget {
  target: LaunchTarget;
  /** Folder the default comes from; `null` when set on the bookmark itself. */
  folderId: number | null;
  folderName: string | null;
}

export interface PageMeta {
  title: string | null;
  /** File name in the favicons dir. */
  favicon: string | null;
}

export interface DesktopSettings {
  /** "" = no global shortcut. */
  globalShortcut: string;
  /** Why the configured shortcut isn't active, if it isn't. */
  shortcutError: string | null;
  closeToTray: boolean;
  autostart: boolean;
  /** Global shortcuts don't work under Wayland. */
  wayland: boolean;
  /** Path of the app's executable, for a desktop-environment shortcut. */
  executable: string;
}

export interface ExportSummary {
  folders: number;
  bookmarks: number;
}

export interface ImportPreview {
  /** "backup" = Link Opener backup; "browser" = a browser's bookmarks.html. */
  kind: "backup" | "browser";
  folders: number;
  bookmarks: number;
  /** When the backup was made (unix ms). */
  exportedAt: number | null;
}

/** "merge" adds to the library; "replace" deletes everything first (backups only). */
export type ImportMode = "merge" | "replace";

export interface ImportSummary {
  foldersCreated: number;
  bookmarksAdded: number;
  duplicatesSkipped: number;
  invalidSkipped: number;
}

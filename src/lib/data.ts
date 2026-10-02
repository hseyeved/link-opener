// Backup, export and import actions, shared by the toolbar menu and Settings.
import * as api from "./api";
import { app } from "./state.svelte";
import { ui } from "./ui.svelte";

/** Local date as YYYY-MM-DD, for file names. */
function today(): string {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function count(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

export async function backUp() {
  await app.run(async () => {
    const path = await api.pickSavePath(`link-opener-backup-${today()}.json`, {
      name: "Link Opener backup",
      extensions: ["json"],
    });
    if (!path) return;
    const done = await api.exportBackup(path);
    ui.notify(`Backed up ${count(done.bookmarks, "bookmark")}.`);
  });
}

export async function exportHtml() {
  await app.run(async () => {
    const path = await api.pickSavePath(`bookmarks-${today()}.html`, {
      name: "Browser bookmarks file",
      extensions: ["html"],
    });
    if (!path) return;
    const done = await api.exportHtml(path);
    ui.notify(`Exported ${count(done.bookmarks, "bookmark")} for browsers.`);
  });
}

/** Asks for a file, then shows what it holds in the import dialog. */
export async function startImport() {
  await app.run(async () => {
    const path = await api.pickImportPath();
    if (!path) return;
    const preview = await api.inspectImport(path);
    ui.closeAll();
    ui.importing = { path, preview };
  });
}

export { count };

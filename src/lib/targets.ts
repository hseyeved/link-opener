// Browser × profile choices, shared by the picker and the default-browser selects.
import type { Browser, LaunchTarget, Profile } from "./types";

export interface TargetOption {
  browser: Browser;
  /** `null` when the browser has a single profile (or none): launch without a profile flag. */
  profile: Profile | null;
}

/** One option per profile for multi-profile browsers, else one per browser. Hidden browsers
 * and profiles are left out unless `includeHidden`. */
export function targetOptions(browsers: Browser[], includeHidden = false): TargetOption[] {
  return browsers.flatMap((browser): TargetOption[] => {
    if (browser.hidden && !includeHidden) return [];
    if (browser.profiles.length > 1) {
      // If hiding leaves one profile, it's still launched explicitly with that profile.
      const profiles = includeHidden ? browser.profiles : browser.profiles.filter((p) => !p.hidden);
      return profiles.map((profile) => ({ browser, profile }));
    }
    return [{ browser, profile: null }];
  });
}

export function optionKey(option: TargetOption): string {
  return `${option.browser.id}/${option.profile?.id ?? ""}`;
}

export function toTarget(option: TargetOption, privateWindow: boolean): LaunchTarget {
  return { browserId: option.browser.id, profileId: option.profile?.id ?? null, private: privateWindow };
}

/** Index of the option for `target`: an exact profile match, else the browser's first option. */
export function findOption(options: TargetOption[], target: LaunchTarget): number {
  const exact = options.findIndex(
    (o) => o.browser.id === target.browserId && (o.profile?.id ?? null) === target.profileId,
  );
  return exact >= 0 ? exact : options.findIndex((o) => o.browser.id === target.browserId);
}

export function optionLabel(option: TargetOption): string {
  return option.profile ? `${option.browser.name} — ${option.profile.name}` : option.browser.name;
}

/** "Microsoft Edge — Work (private)"; falls back to the raw ids for a browser that's gone. */
export function describeTarget(target: LaunchTarget, browsers: Browser[] | null): string {
  const browser = browsers?.find((b) => b.id === target.browserId);
  const profile = browser?.profiles.find((p) => p.id === target.profileId);
  let label = browser?.name ?? `${target.browserId} (not found)`;
  if (target.profileId && (profile || !browser)) label += ` — ${profile?.name ?? target.profileId}`;
  return target.private ? `${label} (private)` : label;
}

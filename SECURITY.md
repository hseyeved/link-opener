# Security policy

## Reporting a vulnerability

Please **don't open a public issue** for security problems. Report them privately through
GitHub: on this repository, go to **Security → Report a vulnerability**. You'll get a reply as
soon as we can, and we'll keep you updated until it's fixed.

Please include what you found, how to reproduce it, and which version and OS you used.

## Supported versions

Fixes go into the latest release. Please update before reporting, if you can.

## What counts

Link Opener reads browser profile lists, starts browsers with a URL, fetches pages to get their
titles and icons, and stores your library locally. Things we'd especially like to hear about:

- A bookmark URL or imported file that makes the app run a command, pass extra flags to a
  browser, or read or write files it shouldn't.
- Script running in the app's window (for example from a page title, a favicon or an imported
  bookmarks file).
- Anything that sends your data anywhere other than the pages you bookmark.

## How the app limits this

- URLs are passed to browsers as a single argument, never through a shell, and URLs starting
  with `-` are refused so they can't act as browser flags.
- The window has a strict Content Security Policy: only the app's own scripts run, and images
  load only from the app and its favicon folder.
- The web view can only call the app's own commands; the global shortcut, launch-at-login and
  window-state features have no web view access.
- Favicons are checked by content to be images and are stored under a hash of their content.

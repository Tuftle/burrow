# Burrow in Wren

Burrow is built into Wren's extension. The same `web/` folder runs as a page there, plus three small pieces of Wren-only code.

| Piece | Where (in the Wren source tree, `browser/extensions/wren`) | What it does |
| --- | --- | --- |
| The page | `pages/burrow/` (copied from `web/` by `tools/sync-to-wren.py`) | The vault itself. Unlock, edit, import. Also tells the background script which logins are open, through `host.js`. |
| Background | `background/vault.js` | Holds the open logins in memory only. Answers the content script and checks each request against the tab's real address. Drops everything on lock or after ten quiet minutes. |
| Content | `content/vault.js` | The small chip on login pages, and "save this login?" after a form is sent. |
| Manifest | `manifest.json`, `pages/palette.js`, `pages/shortcuts.html` | Commands, scripts, a CSP that allows WebAssembly (`'wasm-unsafe-eval'`), palette entries. |

## Keys

| Keys | What |
| --- | --- |
| `alt+shift+v` | open burrow |
| `alt+shift+l` | fill this page's login |
| palette | "open burrow", "fill this page's login from burrow", "lock burrow" |

## How a fill works

1. You unlock the vault in the burrow page. It sends `{id, site, url, user, pass}` for each entry to the background script.
2. A login page loads. The content script asks `vault:matches`. The background script answers with ids and usernames for entries that match the tab's address, never passwords.
3. You click the chip. The content script asks `vault:fill` with an id. The background script checks the match again against `sender.tab.url` and only then returns the login.

## How a save works

1. You send a login form. The content script sends the username and password to the background script, which remembers them for 30 seconds, only if the vault is open and the login is not already saved.
2. On the next page, or 1.5 seconds later, the content script asks whether there is an offer and shows "save ... ?".
3. You click it. The background script opens a small burrow window with the login waiting. You unlock it if it is locked, check the details, and save. The password is never put in a URL.

## Updating

```bash
python tools/sync-to-wren.py
C:\Users\6wntt\wrentools\run-full.sh build.log build faster   # UI-only rebuild
python C:\Users\6wntt\wrentools\make_overlay.py                # refresh the overlay repo
```

## Tested

Against a dev build with a throwaway profile and a local login page: creating a vault (Argon2 under the CSP), the state reaching the background script, filling a login through the real content script, the save offer after a real page change, and the pending login arriving prefilled in the burrow window.

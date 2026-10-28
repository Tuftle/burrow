# Burrow as a browser extension

The same app, in Firefox and Chrome. `web/` is copied unchanged into the extension's `app/` folder, and two small scripts
fill logins on web pages: `extension/src/background.js` and `extension/src/content.js`. They are the same pieces that run
inside [Wren](WREN.md), made to work in both browsers.

## Build

```
python tools/build-extension.py
```

| Output | What |
| :-- | :-- |
| `extension/dist/extension-firefox/` and `Burrow-Firefox.zip` | Manifest V2, with an event page. Needs Firefox 115 or newer. |
| `extension/dist/extension-chrome/` and `Burrow-Chrome.zip` | Manifest V3, with a service worker. Needs Chrome 110 or newer. |

## Try it

Download `Burrow-Firefox.zip` or `Burrow-Chrome.zip` from the [latest release](https://github.com/Tuftle/burrow/releases/latest),
or build them yourself as above.

**Firefox:** open `about:debugging`, choose **This Firefox**, then **Load Temporary Add-on**, and pick `manifest.json` in
`extension-firefox` (or choose the zip itself). Temporary add-ons are removed when Firefox closes. Firefox only keeps an add-on for good when it is
signed by Mozilla, which means listing it on addons.mozilla.org.

**Chrome and Edge:** open `chrome://extensions`, turn on **Developer mode**, choose **Load unpacked** and pick
`extension-chrome`, or the folder you got by unzipping `Burrow-Chrome.zip`.

Click the toolbar button, or press `Alt+Shift+V`, to open Burrow. Make a vault, or restore a backup. Then open a login page:
a small chip offers the login for that site. `Alt+Shift+L` fills it without the chip. After you sign in somewhere new, Burrow
offers to save the login.

## What it asks for

| Permission | Why |
| :-- | :-- |
| Run on `https://` pages, `localhost` and `127.0.0.1` | To show the chip on login pages. This is the one broad permission, and it is what filling logins needs. |
| `storage` | Keeps the open logins in `storage.session`, see below. |
| `alarms` | Locks the vault after ten quiet minutes, even if the service worker was stopped in between. |
| `clipboardWrite` | Copying a password, which is then cleared after 20 seconds. |

It does not ask for `tabs`, history, bookmarks, cookies or downloads, and it makes no network requests of its own.

## How it keeps the logins

- The vault page unlocks the vault, and sends the open logins to the background script. They are kept in
  `storage.session`, which lives in memory only, is never written to disk, is cleared when the browser closes, and cannot be
  read by content scripts. Plain variables would not survive Chrome stopping its service worker.
- A web page never gets a login by asking for it. The content script asks for logins that match its own address, the
  background script answers with names only, and a password is handed over only after a real click, checked again against the
  page's own address.
- The chip lives in a closed shadow root and ignores clicks that did not come from the person.
- Locking, from the button, `Ctrl+L` or the ten minute timer, clears everything above.

## Tested

In a Firefox-based browser and in Chrome 156, with a local login page, using real mouse clicks:

- the vault is made (Argon2 runs as WebAssembly under the extension's content security policy)
- clicking the chip fills the login
- a page on another host, or a locked vault, gets nothing
- after Chrome's service worker is stopped, filling still works
- clicking the save offer opens a Burrow window with the new login ready, and no secret in its address

## For the add-on stores

Mozilla's reviewers ask for the source of anything that is not plain: the only such file is `app/vendor/argon2.js`, which is
`hash-wasm` 4.11.0 (`dist/argon2.umd.min.js`) unmodified. A test checks that it is byte for byte the file from the pinned npm
package.

# burrow

A quiet password manager from [tuftle](https://tuftle.vercel.app). Your logins live in one scrambled file that only your master password can open. There are no accounts, no telemetry and no server in the way.

It runs three ways, from the same code and the same vault format:

| Where | What | Vault is kept |
| --- | --- | --- |
| The web | `web/`, plain HTML and JavaScript, no framework | in the browser (IndexedDB) |
| Windows app | `desktop/`, a small Tauri window around the same page | in `%APPDATA%\Burrow\vault.json` |
| Windows, portable | the same exe, renamed so its name contains `portable` | in a `burrow-data` folder next to the exe |
| Wren | built into the browser, `alt+shift+v` | in the extension's storage |

## What it does

- Encrypts on your computer with Argon2id and AES-256-GCM. A wrong password and a damaged file are both refused.
- A recovery key, shown once when you make the vault. There is no reset: lose the password and the key, and the vault is gone.
- Entries with site, web address, username, password, notes and an optional one-time code secret.
- A generator for random characters or plain-word passphrases (256 words, 8 bits each).
- One-time codes (RFC 6238), with a countdown.
- A checkup for weak and reused passwords. An opt-in breach check sends only the first five characters of a SHA-1 fingerprint to haveibeenpwned.com.
- Import from Chrome, Edge and Firefox (CSV) and Bitwarden (JSON). Export an encrypted backup, or a plain CSV behind a warning.
- Auto-lock after five quiet minutes, `ctrl+l` to lock, `ctrl+k` to search, clipboard cleared after 20 seconds.
- In Wren: a small chip fills the login for the page you are on, and offers to save new ones. See [docs/WREN.md](docs/WREN.md).

## Layout

```
web/         the app (index.html, app.js, crypto.js, generator.js, importers.js, totp.js, health.js, match.js, store.js, host.js)
tests/       node --test, 38 tests
desktop/     Tauri shell (Rust) that embeds web/ and keeps the vault in a file
installer/   Burrow-Setup.exe: a small egui window, no NSIS. make.py joins it with the app
tools/       sync-to-wren.py copies web/ into the Wren source tree
docs/        THREAT-MODEL.md, WREN.md
```

## Run it

```bash
npm install
npm test                       # 38 tests, no browser needed
cd web && python -m http.server 8731   # then open http://localhost:8731
```

Build the Windows files (needs Rust and the Visual Studio build tools):

```bash
cd desktop   && cargo build --release --features custom-protocol
cd ../installer && cargo build --release && python make.py
```

`make.py` writes `Burrow-Setup.exe`, `Burrow-Portable.exe` and `SHA256SUMS.txt` to `I:\Documents\code\burrow-release`. The installer carries `burrow.exe` on its end, puts it in `%LOCALAPPDATA%\Programs\Burrow`, adds the shortcuts and an entry in Windows' list of apps, and takes it all away again with `--uninstall`. Previews of every screen: `burrow-setup.exe --preview-installing=0.5`, `--preview-done`, `--preview-removed`, with `--light` or `--dark`.

## Not done yet

- Sync between computers. Planned as an optional encrypted blob on a private store, with no accounts. For now, copy the encrypted backup, or keep the vault file in a folder you already sync.
- Code signing, so Windows SmartScreen will warn on first run.
- macOS and Linux builds.

## Licence

Not chosen yet. MPL 2.0, like Wren, is the likely pick.

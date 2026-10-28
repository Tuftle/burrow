# Threat model

What burrow protects, what it does not, and why. Written plainly on purpose.

## The design in one paragraph

Your master password is stretched with Argon2id (64 MiB, 3 passes) into a key. That key unwraps a random 256-bit data key, and the data key encrypts the vault with AES-256-GCM. The recovery key unwraps a second copy of the same data key. Changing the master password only rewraps that one data key. Everything happens on your device. The file that is stored holds ciphertext, the Argon2 settings and a salt, and nothing else.

## What it protects against

| Threat | How |
| --- | --- |
| Someone gets the vault file (stolen laptop, copied backup, a synced folder) | They have to guess the master password through Argon2id, one slow guess at a time. A long passphrase makes that impractical. |
| The file is changed or damaged | AES-GCM authenticates every byte. A changed file is refused, not half opened. |
| Someone looks at the file for site names or usernames | There are none in it. Only ciphertext is stored, entries included. |
| A web page asks for logins that are not its own | In Wren, the background script matches the page's own address, strictly: the same host or a subdomain, over https (or a local test page). `evil-example.com` and `example.com.evil.net` never match for `example.com`. The check is repeated on every fill. |
| A web page clicks the fill button for you | The chip lives in a closed shadow root and ignores clicks that did not come from you. |
| A password lingering on the clipboard | Cleared after 20 seconds. |
| Walking away from an open vault | Locks after five quiet minutes, or at once with the lock button or `ctrl+l`. In Wren the open logins are also dropped after ten quiet minutes, and the panic key closes everything. |
| The breach check leaking a password | Only the first five characters of a SHA-1 fingerprint are sent, and only if you turn it on. |
| A tampered update | The installed app only downloads from Burrow's own GitHub releases, and checks the SHA-256 that was announced with the update before running anything. A wrong checksum or a foreign address is refused and the download thrown away. |
| Logins left in a browser | In the extension they live in `storage.session`: memory only, never written to disk, gone when the browser closes, and not readable by content scripts. |
| Tracking | There is no server, no account and no analytics. The only network calls are the breach check you ask for, and in the installed Windows app a once-a-day look for a newer version (one public file, nothing about you, and it can be turned off). |

## What it does not protect against

- **A computer that is already compromised.** A keylogger, a screen reader or malware running as you can see the master password as you type it and the passwords as they are shown. No password manager can fix that.
- **A weak master password.** Argon2id slows guessing, it cannot rescue `password123`. Use a long passphrase.
- **You, forgetting both secrets.** There is no reset and no backdoor. That is the point, and also the risk. Keep the recovery key somewhere safe and separate.
- **The page you fill into.** Once a login is filled, the page's own scripts can read the field like any other. Fill only on sites you trust, which is why nothing is filled until you click.
- **Memory inspection while unlocked.** While open, the decrypted entries exist in memory. Locking drops them, but JavaScript cannot guarantee that every copy is wiped immediately.
- **Phishing that you click through.** Matching is strict, so a look-alike site gets no login from burrow. That is a useful warning, but it is not a defence if you then type the password by hand.

## Updates, honestly

The update check trusts two things: the HTTPS address `tuftle.vercel.app/burrow/latest.json`, and Burrow's GitHub releases.
The checksum in the file proves that the download is the one that was announced, but the installer is not code-signed yet,
so there is no second, independent signature. Someone who controlled both the website and the GitHub account could announce
a bad update. Code signing is planned, and it closes that gap.

## Choices that deserve a second look

- Argon2id comes from the vendored `hash-wasm` build (4.11.0, WebAssembly). It is the one piece of crypto that is not the browser's own WebCrypto.
- The entry-level fields (`site`, `url`) are inside the encrypted blob. The blob's size reveals roughly how many entries you have.
- The recovery key is 160 bits, shown as base32. It is derived with HKDF, not Argon2, because it is already random.
- The browser extension runs on every `https://` page so that it can show the chip. It only ever reads the page's own address
  and the login form, and it hands a login to a page only after a real click on its own chip.
- The portable and desktop builds keep the vault in a plain file. File permissions are those of your user account.

## Reporting a problem

Write to tuftle@proton.me. Please include steps, not a real vault.

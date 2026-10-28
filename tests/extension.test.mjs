import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import vm from "node:vm";
import { matches, entryHost } from "../web/match.js";

const out = mkdtempSync(join(tmpdir(), "burrow-ext-"));
const built = spawnSync("python", ["tools/build-extension.py", out], { encoding: "utf8" });

test("both extensions build", () => {
  assert.equal(built.status, 0, built.stderr);
  for (const b of ["firefox", "chrome"]) {
    assert.ok(existsSync(join(out, `extension-${b}`, "manifest.json")));
    assert.ok(existsSync(join(out, `Burrow-${b[0].toUpperCase() + b.slice(1)}.zip`)));
  }
});

test("manifests: firefox is v2 with an id, chrome is v3 with a service worker", () => {
  const f = JSON.parse(readFileSync(join(out, "extension-firefox", "manifest.json"), "utf8"));
  const c = JSON.parse(readFileSync(join(out, "extension-chrome", "manifest.json"), "utf8"));
  assert.equal(f.manifest_version, 2);
  assert.ok(f.browser_specific_settings.gecko.id.includes("@"));
  assert.equal(c.manifest_version, 3);
  assert.equal(c.background.service_worker, "background.js");
  for (const m of [f, c]) {
    assert.equal(m.version, f.version);
    assert.ok(!JSON.stringify(m).includes("__VERSION__"));
    assert.deepEqual([...m.permissions].sort(), ["alarms", "clipboardWrite", "storage"]);
  }
});

test("the classic matcher behaves exactly like the module", () => {
  const ctx = { self: {}, URL };
  vm.createContext(ctx);
  vm.runInContext(readFileSync(join(out, "extension-chrome", "match.classic.js"), "utf8"), ctx);
  const { matches: m2, entryHost: h2 } = ctx.self.burrowMatch;
  const entries = [{ url: "https://example.com" }, { site: "example.org" }, { site: "My bank" }, { url: "http://localhost:3000" }];
  const pages = [
    "https://example.com/login", "https://www.example.com/", "https://evil-example.com/", "https://example.com.evil.net/",
    "http://example.com/", "https://example.org/a", "http://localhost:3000/x", "javascript:alert(1)", "nonsense",
  ];
  for (const e of entries) {
    assert.equal(h2(e), entryHost(e));
    for (const p of pages) assert.equal(m2(e, p), matches(e, p), `${JSON.stringify(e)} on ${p}`);
  }
});

test("the app inside the extension is the web app, unchanged", () => {
  for (const f of ["app.js", "crypto.js", "match.js", "index.html", "vendor/argon2.js"]) {
    assert.ok(readFileSync(join(out, "extension-firefox", "app", f)).equals(readFileSync(new URL(`../web/${f}`, import.meta.url))), f);
  }
});

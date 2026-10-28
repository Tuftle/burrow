import { test } from "node:test";
import assert from "node:assert/strict";
import { isNewer, parseVersion, readFeed, checkForUpdate } from "../web/update.js";

const SHA = "2dbf42822cb96d6e8a17d23177b4251f5cfca78f954d20a43dfebd8045bea5a8";
const good = {
  version: "0.3.0",
  notes: "a fix",
  url: "https://github.com/Tuftle/burrow/releases/download/v0.3.0/Burrow-Setup.exe",
  sha256: SHA,
  portableUrl: "https://github.com/Tuftle/burrow/releases/download/v0.3.0/Burrow-Portable.exe",
};

test("versions compare as numbers, not text", () => {
  assert.ok(isNewer("0.10.0", "0.9.0"));
  assert.ok(isNewer("1.0.0", "0.99.99"));
  assert.ok(isNewer("0.2.1", "0.2.0"));
  assert.ok(!isNewer("0.2.0", "0.2.0"));
  assert.ok(!isNewer("0.1.9", "0.2.0"));
});

test("junk versions are never newer", () => {
  assert.equal(parseVersion("1.2"), null);
  assert.equal(parseVersion("1.2.3-beta"), null);
  assert.equal(parseVersion("v1.2.3"), null);
  assert.ok(!isNewer("9.9.9-evil", "0.1.0"));
  assert.ok(!isNewer("not a version", "0.1.0"));
  assert.ok(!isNewer("1.0.0", "unknown"));
});

test("a good feed is accepted and tidied", () => {
  const f = readFeed({ ...good, sha256: SHA.toUpperCase(), notes: "x".repeat(900) });
  assert.equal(f.sha256, SHA);
  assert.equal(f.notes.length, 400);
});

test("feeds that point anywhere but burrow's releases are refused", () => {
  const bad = (patch) => assert.throws(() => readFeed({ ...good, ...patch }));
  bad({ url: "https://evil.example/Burrow-Setup.exe" });
  bad({ url: "http://github.com/Tuftle/burrow/releases/download/v1.0.0/x.exe" });
  bad({ url: "https://github.com/Tuftle/wren/releases/download/v1/x.exe" });
  bad({ url: "https://github.com/Tuftle/burrow/releases/../../evil/x.exe" });
  bad({ url: "https://github.com/Tuftle/burrow/releases/x.exe?token=1" });
  bad({ portableUrl: "https://evil.example/p.exe" });
});

test("feeds with a missing or bad checksum or version are refused", () => {
  assert.throws(() => readFeed({ ...good, sha256: "abc" }));
  assert.throws(() => readFeed({ ...good, sha256: undefined }));
  assert.throws(() => readFeed({ ...good, version: "latest" }));
  assert.throws(() => readFeed(null));
  assert.throws(() => readFeed("text"));
});

test("checkForUpdate reports available or not, and fails cleanly", async () => {
  const feed = (json, ok = true) => async () => ({ ok, json: async () => json });
  assert.equal((await checkForUpdate("0.2.0", feed(good))).available, true);
  assert.equal((await checkForUpdate("0.3.0", feed(good))).available, false);
  await assert.rejects(checkForUpdate("0.2.0", feed(good, false)), /could not be fetched/);
  await assert.rejects(checkForUpdate("0.2.0", feed({ ...good, url: "https://evil.example/x" })));
});

test("the request sends no cookies and no referrer", async () => {
  let seen;
  await checkForUpdate("0.2.0", async (url, init) => { seen = init; return { ok: true, json: async () => good }; });
  assert.equal(seen.credentials, "omit");
  assert.equal(seen.referrerPolicy, "no-referrer");
});

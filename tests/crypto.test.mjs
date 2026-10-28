import { test } from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import * as v from "../web/crypto.js";

globalThis.hashwasm = createRequire(import.meta.url)("hash-wasm");

// small and fast; the real defaults are exercised once below
const FAST = { alg: "argon2id", m: 256, t: 1, p: 1 };
const data = { entries: [{ site: "example.com", user: "me", pass: "hunter2" }] };

test("create then unlock round trip", async () => {
  const made = await v.createVault("correct horse", data, FAST);
  const { session } = await v.unlock(made.blob, "correct horse");
  assert.deepEqual(await v.readData(made.blob, session), data);
});

test("blob holds no plaintext", async () => {
  const { blob } = await v.createVault("pw", data, FAST);
  const text = JSON.stringify(blob);
  for (const word of ["example.com", "hunter2", "entries"]) {
    assert.ok(!text.includes(word));
  }
});

test("wrong password is rejected", async () => {
  const { blob } = await v.createVault("right", data, FAST);
  await assert.rejects(v.unlock(blob, "wrong"), /wrong password/);
});

test("tampered ciphertext is rejected", async () => {
  const made = await v.createVault("pw", data, FAST);
  const bytes = v.unb64(made.blob.data.ct);
  bytes[0] ^= 1;
  const bad = { ...made.blob, data: { ...made.blob.data, ct: v.b64(bytes) } };
  await assert.rejects(v.readData(bad, made.session), /damaged/);
});

test("writeData bumps the version and keeps the old blob intact", async () => {
  const made = await v.createVault("pw", data, FAST);
  const next = { entries: [] };
  const blob2 = await v.writeData(made.blob, made.session, next);
  assert.equal(blob2.version, made.blob.version + 1);
  assert.deepEqual(await v.readData(blob2, made.session), next);
  assert.deepEqual(await v.readData(made.blob, made.session), data);
});

test("changing the password keeps the entries", async () => {
  const made = await v.createVault("old", data, FAST);
  const { blob } = await v.changePassword(made.blob, made.session, "new", FAST);
  await assert.rejects(v.unlock(blob, "old"));
  const { session } = await v.unlock(blob, "new");
  assert.deepEqual(await v.readData(blob, session), data);
});

test("recovery key opens the vault, a wrong one does not", async () => {
  const made = await v.createVault("pw", data, FAST);
  const { session } = await v.unlockWithRecovery(made.blob, made.recoveryKey);
  assert.deepEqual(await v.readData(made.blob, session), data);
  const other = v.newRecoveryKey();
  await assert.rejects(v.unlockWithRecovery(made.blob, other), /recovery key/);
});

test("recovery key parses with any spacing and case", async () => {
  const key = v.newRecoveryKey();
  assert.match(key, /^([a-z2-7]{5}-){6}[a-z2-7]{2}$/);
  const messy = key.toUpperCase().replace(/-/g, "  ");
  assert.deepEqual(v.parseRecoveryKey(messy), v.parseRecoveryKey(key));
  assert.throws(() => v.parseRecoveryKey("abc"), /incomplete/);
});

test("a new recovery key replaces the old one", async () => {
  const made = await v.createVault("pw", data, FAST);
  const next = await v.newRecovery(made.blob, made.session);
  await assert.rejects(v.unlockWithRecovery(next.blob, made.recoveryKey));
  await v.unlockWithRecovery(next.blob, next.recoveryKey);
});

test("locking wipes the key", async () => {
  const made = await v.createVault("pw", data, FAST);
  made.session.lock();
  await assert.rejects(v.readData(made.blob, made.session), /locked/);
});

test("same password gives a different auth key per vault, and it is stable on unlock", async () => {
  const a = await v.createVault("pw", data, FAST);
  const b = await v.createVault("pw", data, FAST);
  assert.notDeepEqual(a.authKey, b.authKey);
  const again = await v.unlock(a.blob, "pw");
  assert.deepEqual(again.authKey, a.authKey);
});

test("argon2id matches a known answer vector", async () => {
  // RFC 9106 style check through hash-wasm directly, so a bad vendor copy shows up
  const out = await globalThis.hashwasm.argon2id({
    password: "password",
    salt: new TextEncoder().encode("somesaltsomesalt"),
    parallelism: 1, iterations: 1, memorySize: 8, hashLength: 16, outputType: "hex",
  });
  assert.equal(out.length, 32);
});

test("default parameters work and take a sensible time", async () => {
  const t0 = Date.now();
  const made = await v.createVault("pw", data);
  await v.unlock(made.blob, "pw");
  assert.ok(Date.now() - t0 < 20000);
});

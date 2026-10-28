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

test("argon2id matches the reference implementation's published vector", async () => {
  // from the Argon2 reference test suite:
  // $argon2id$v=19$m=65536,t=2,p=1$c29tZXNhbHQ$CTFhFdXPJO1aFaMaO6Mm5c8y7cJHAph8ArZWb2GRPPc
  const out = await globalThis.hashwasm.argon2id({
    password: "password",
    salt: "somesalt",
    parallelism: 1, iterations: 2, memorySize: 65536, hashLength: 32, outputType: "hex",
  });
  assert.equal(out, "09316115d5cf24ed5a15a31a3ba326e5cf32edc24702987c02b6566f61913cf7");
  assert.equal(out, Buffer.from("CTFhFdXPJO1aFaMaO6Mm5c8y7cJHAph8ArZWb2GRPPc", "base64").toString("hex"));
});

test("the vendored argon2 file is the one from the pinned npm package", async () => {
  const { readFile } = await import("node:fs/promises");
  const vendored = await readFile(new URL("../web/vendor/argon2.js", import.meta.url));
  const original = await readFile(new URL("../node_modules/hash-wasm/dist/argon2.umd.min.js", import.meta.url));
  assert.ok(vendored.equals(original));
});

test("the vault's own key derivation gives the same key for the same inputs, and different for others", async () => {
  const a = await v.createVault("pw", data, FAST);
  const b = await v.unlock(a.blob, "pw");
  assert.deepEqual(b.authKey, a.authKey);
  const c = await v.changePassword(a.blob, a.session, "pw", FAST); // new salt
  assert.notDeepEqual(c.authKey, a.authKey);
});

test("default parameters work and take a sensible time", async () => {
  const t0 = Date.now();
  const made = await v.createVault("pw", data);
  await v.unlock(made.blob, "pw");
  assert.ok(Date.now() - t0 < 20000);
});

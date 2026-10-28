import { test } from "node:test";
import assert from "node:assert/strict";
import * as imp from "../web/importers.js";
import * as totp from "../web/totp.js";
import * as health from "../web/health.js";

test("csv handles quotes, commas, newlines and a BOM", () => {
  const rows = imp.parseCsv('﻿a,b\r\n"x, y","say ""hi"""\r\n"line\nbreak",z\r\n');
  assert.deepEqual(rows, [["a", "b"], ["x, y", 'say "hi"'], ["line\nbreak", "z"]]);
});

test("csv round trip", () => {
  const rows = [["a", 'b"c'], ["d,e", "f\ng"]];
  assert.deepEqual(imp.parseCsv(imp.toCsv(rows)), rows);
});

test("chrome export", () => {
  const e = imp.fromBrowserCsv("name,url,username,password,note\nGitHub,https://github.com/login,me,pw1,hello\n");
  assert.equal(e.length, 1);
  assert.deepEqual([e[0].site, e[0].user, e[0].pass, e[0].notes], ["GitHub", "me", "pw1", "hello"]);
});

test("firefox export falls back to the host name", () => {
  const e = imp.fromBrowserCsv('"url","username","password","httpRealm"\n"https://www.example.com/a","me","pw",""\n');
  assert.equal(e[0].site, "example.com");
});

test("bitwarden export keeps logins only", () => {
  const json = JSON.stringify({ items: [
    { type: 1, name: "Site", notes: "n", login: { username: "u", password: "p", totp: "JBSWY3DPEHPK3PXP", uris: [{ uri: "https://s.com" }] } },
    { type: 2, name: "A secure note" },
  ] });
  const e = imp.fromBitwarden(json);
  assert.equal(e.length, 1);
  assert.equal(e[0].totp, "JBSWY3DPEHPK3PXP");
});

test("encrypted bitwarden and junk are refused", () => {
  assert.throws(() => imp.fromBitwarden('{"encrypted":true}'), /encrypted/);
  assert.throws(() => imp.fromBrowserCsv("a,b\n1,2\n"), /browser/);
  assert.throws(() => imp.fromBitwarden("nope"), /json/);
});

test("newOnly skips what is already there", () => {
  const a = { site: "s", user: "u", pass: "p" };
  assert.equal(imp.newOnly([a], [{ ...a }, { ...a, pass: "q" }, { ...a, pass: "q" }]).length, 1);
});

test("totp matches the rfc 6238 sha1 vectors", async () => {
  const secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"; // "12345678901234567890"
  const cases = [[59, "94287082"], [1111111109, "07081804"], [1111111111, "14050471"], [20000000000, "65353130"]];
  for (const [t, want] of cases) {
    assert.equal(await totp.code(secret, t * 1000, { digits: 8 }), want);
  }
});

test("totp reads otpauth links and loose secrets", async () => {
  const a = await totp.code("otpauth://totp/x?secret=jbsw y3dp&issuer=y".replace(" ", ""), 0);
  const b = await totp.code("JBSW-Y3DP", 0);
  assert.equal(a, b);
  await assert.rejects(totp.code("not*base32", 0));
});

test("report finds weak and reused passwords", () => {
  const r = health.report([
    { site: "a", pass: "abc" },
    { site: "b", pass: "Xk9#mP2$vL7qRt4!" },
    { site: "c", pass: "Xk9#mP2$vL7qRt4!" },
  ]);
  assert.equal(r.weak.length, 1);
  assert.equal(r.reused.length, 1);
  assert.equal(r.reused[0].length, 2);
});

test("breach check sends only five characters and finds the match", async () => {
  // sha1("password") = 5BAA61E4C9B93F3F0682250B6CF8331B7EE68FD8
  const seen = [];
  const fake = async (url) => {
    seen.push(url);
    return { ok: true, text: async () => "1E4C9B93F3F0682250B6CF8331B7EE68FD8:9545824\r\n0000000000000000000000000000000000A:0" };
  };
  const hits = await health.breached([{ site: "a", pass: "password" }, { site: "b", pass: "zq81!unique-long-thing" }], fake);
  assert.equal(hits.length, 1);
  assert.equal(hits[0].entry.site, "a");
  assert.ok(seen.every((u) => /\/range\/[0-9A-F]{5}$/.test(u)));
  assert.ok(seen.every((u) => u.split("/range/")[1].length === 5));
});

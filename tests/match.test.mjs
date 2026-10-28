import { test } from "node:test";
import assert from "node:assert/strict";
import { entryHost, matches } from "../web/match.js";

test("entry host comes from the address, else a host-like name", () => {
  assert.equal(entryHost({ url: "https://www.GitHub.com/login", site: "GitHub" }), "github.com");
  assert.equal(entryHost({ site: "example.com" }), "example.com");
  assert.equal(entryHost({ site: "https://news.example.org/a" }), "news.example.org");
  assert.equal(entryHost({ site: "My bank" }), "");
  assert.equal(entryHost({ site: "com" }), "");
});

test("same host and subdomains match", () => {
  const e = { site: "x", url: "https://example.com" };
  assert.ok(matches(e, "https://example.com/login"));
  assert.ok(matches(e, "https://www.example.com/"));
  assert.ok(matches(e, "https://accounts.example.com/signin"));
});

test("look-alike hosts never match", () => {
  const e = { site: "x", url: "https://example.com" };
  assert.ok(!matches(e, "https://evil-example.com/"));
  assert.ok(!matches(e, "https://example.com.evil.net/"));
  assert.ok(!matches(e, "https://notexample.com/"));
  assert.ok(!matches(e, "https://example.co/"));
  assert.ok(!matches(e, "https://evil.net/?u=example.com"));
  assert.ok(!matches(e, "https://evil.net/example.com"));
});

test("a subdomain entry does not match its parent", () => {
  assert.ok(!matches({ url: "https://accounts.example.com" }, "https://example.com/"));
});

test("plain http is refused except on a local test page", () => {
  const e = { url: "https://example.com" };
  assert.ok(!matches(e, "http://example.com/login"));
  assert.ok(matches({ url: "http://localhost:3000" }, "http://localhost:3000/login"));
  assert.ok(matches({ url: "http://127.0.0.1" }, "http://127.0.0.1:8080/"));
});

test("junk and other schemes never match", () => {
  const e = { url: "https://example.com" };
  assert.ok(!matches(e, "not a url"));
  assert.ok(!matches(e, "file:///C:/example.com"));
  assert.ok(!matches(e, "javascript:alert(1)"));
  assert.ok(!matches({ site: "" }, "https://example.com"));
});

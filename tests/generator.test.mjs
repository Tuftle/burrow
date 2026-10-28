import { test } from "node:test";
import assert from "node:assert/strict";
import { generate, strength } from "../web/generator.js";

test("length and character kinds are respected", () => {
  for (let i = 0; i < 50; i++) {
    const p = generate({ length: 12 });
    assert.equal(p.length, 12);
    assert.match(p, /[a-z]/);
    assert.match(p, /[A-Z]/);
    assert.match(p, /[2-9]/);
    assert.match(p, /[^a-zA-Z0-9]/);
  }
});

test("turned off kinds never appear", () => {
  for (let i = 0; i < 50; i++) {
    assert.match(generate({ length: 30, upper: false, symbols: false, digits: false }), /^[a-z]+$/);
  }
});

test("needs at least one kind", () => {
  assert.throws(() => generate({ lower: false, upper: false, digits: false, symbols: false }));
});

test("passwords differ", () => {
  assert.notEqual(generate(), generate());
});

test("strength ladder", () => {
  assert.equal(strength("abc").label, "weak");
  assert.equal(strength(generate({ length: 24 })).label, "strong");
});

import { passphrase, passphraseBits } from "../web/generator.js";
import { WORDS } from "../web/words.js";

test("word list is 256 distinct plain words", () => {
  assert.equal(WORDS.length, 256);
  assert.equal(new Set(WORDS).size, 256);
  assert.ok(WORDS.every((w) => /^[a-z]{2,8}$/.test(w)));
});

test("passphrase has the asked number of words from the list", () => {
  const p = passphrase({ words: 6 });
  const parts = p.split("-");
  assert.equal(parts.length, 6);
  assert.ok(parts.every((w) => WORDS.includes(w)));
  assert.notEqual(passphrase(), passphrase());
});

test("passphrase options and limits", () => {
  assert.match(passphrase({ words: 5, capitalize: true }), /^([A-Z][a-z]+-){4}[A-Z][a-z]+$/);
  assert.match(passphrase({ words: 5, number: true }), /-\d{1,2}$/);
  assert.equal(passphrase({ words: 1 }).split("-").length, 4);
  assert.equal(passphrase({ words: 99 }).split("-").length, 12);
  assert.equal(passphraseBits(7), 56);
});

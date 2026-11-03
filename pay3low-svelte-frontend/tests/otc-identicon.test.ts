import test from "node:test";
import assert from "node:assert/strict";
import { identiconDataUri, identiconSvg, md5Hex } from "../src/lib/otc/identicon.ts";

test("identicon hashing matches MD5", () => {
  assert.equal(md5Hex(""), "d41d8cd98f00b204e9800998ecf8427e");
  assert.equal(md5Hex("abc"), "900150983cd24fb0d6963f7d28e17f72");
  assert.equal(md5Hex("0:abcdef"), "49f5785fafa81a7226ad9d8992569730");
});

test("identicon is deterministic, case-insensitive and transparent", () => {
  const address = "0:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  const avatar = identiconSvg(address);
  assert.equal(avatar, identiconSvg(address.toLowerCase()));
  assert.notEqual(avatar, identiconSvg("0:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
  assert.match(avatar, /^<svg xmlns="http:\/\/www\.w3\.org\/2000\/svg"/);
  assert.match(avatar, /fill="rgb\(\d+,\d+,\d+\)"/);
  assert.doesNotMatch(avatar, /fill="#f0f0f0"/);
  assert.doesNotMatch(avatar, /<rect width="420" height="420"/);
});

test("identicon data URI can be used directly as an image source", () => {
  const uri = identiconDataUri("0:abc");
  assert.ok(uri.startsWith("data:image/svg+xml,"));
  assert.ok(uri.includes("%3Csvg"));
  assert.ok(!uri.includes("%23f0f0f0"));
});

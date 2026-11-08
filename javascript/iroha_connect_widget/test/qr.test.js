// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";

import {
  QrEncodeError,
  alignmentPatternPositions,
  dataCodewordCapacity,
  encodeQr,
  qrSvgPath,
  qrToSvg,
  rawDataModuleCount,
} from "../src/qr.js";

const WALLET_URI =
  "iroha://connect?sid=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" +
  "&network_id=hash%3AA5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5%2395D7" +
  "&app_pk=BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB&nonce=CCCCCCCCCCCCCCCCCCCCCC" +
  "&node=https%3A%2F%2Ftaira.sora.org&v=1&role=wallet" +
  "&token=DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD&relay=EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE";

// Module digests cross-checked bit for bit against the `qrcode` reference
// encoder with the same version, level and mask, and decoded with zxing.
const PINNED = [
  { text: "https://taira.sora.org", level: "M", version: 2, mask: 2, digest: "26cc221497a9ad0d72193db780537aae890387cf056bb43f7177d02d8000622a" },
  { text: "a", level: "L", version: 1, mask: 0, digest: "e0eaa9451b23803acd598cdcd8d99adde8caa43b11da279e22533be2510d2a6d" },
  { text: "IrohaConnect 🌸 桜の花びら", level: "Q", version: 4, mask: 4, digest: "a4a533555503fcd2afc2276d84da377a21a2c1b023187a8c74cf874ee9295f48" },
  { text: WALLET_URI, level: "M", version: 15, mask: 4, digest: "47ec5271d2802b2d56e5854b52795ef68c24d25bec2e0f4e3a1d7b705e80b5bd" },
  { text: "x".repeat(1200), level: "H", version: 39, mask: 0, digest: "28d0ee2715d929082090e469e0a45798aa0b5046d296d7355a632982a9006d93" },
];

const LEVEL_FORMAT_BITS = { L: 1, M: 0, Q: 3, H: 2 };

function readFormatInfo(qr) {
  const bit = (x, y) => (qr.isDark(x, y) ? 1 : 0);
  let first = 0;
  for (let i = 0; i <= 5; i += 1) first |= bit(8, i) << i;
  first |= bit(8, 7) << 6;
  first |= bit(8, 8) << 7;
  first |= bit(7, 8) << 8;
  for (let i = 9; i < 15; i += 1) first |= bit(14 - i, 8) << i;
  let second = 0;
  for (let i = 0; i < 8; i += 1) second |= bit(qr.size - 1 - i, 8) << i;
  for (let i = 8; i < 15; i += 1) second |= bit(8, qr.size - 15 + i) << i;
  return { first, second };
}

function bchFormat(data) {
  let remainder = data;
  for (let i = 0; i < 10; i += 1) remainder = (remainder << 1) ^ ((remainder >>> 9) * 0x537);
  return ((data << 10) | remainder) ^ 0x5412;
}

function assertFinder(qr, left, top) {
  for (let dy = 0; dy < 7; dy += 1) {
    for (let dx = 0; dx < 7; dx += 1) {
      const ring = Math.max(Math.abs(dx - 3), Math.abs(dy - 3));
      assert.equal(qr.isDark(left + dx, top + dy), ring !== 2, `finder module ${left + dx},${top + dy}`);
    }
  }
}

test("pinned symbols match the reference encoder", () => {
  for (const pinned of PINNED) {
    const qr = encodeQr(pinned.text, { level: pinned.level });
    assert.equal(qr.version, pinned.version, pinned.text.slice(0, 24));
    assert.equal(qr.mask, pinned.mask, pinned.text.slice(0, 24));
    assert.equal(qr.size, pinned.version * 4 + 17);
    assert.equal(createHash("sha256").update(qr.modules).digest("hex"), pinned.digest);
  }
});

test("function patterns, format and version information are well formed", () => {
  for (const [text, level, minVersion] of [
    ["a", "L", 1],
    ["https://taira.sora.org", "M", 6],
    [WALLET_URI, "Q", 1],
    [WALLET_URI, "H", 32],
  ]) {
    const qr = encodeQr(text, { level, minVersion });
    assertFinder(qr, 0, 0);
    assertFinder(qr, qr.size - 7, 0);
    assertFinder(qr, 0, qr.size - 7);
    for (let i = 8; i < qr.size - 8; i += 1) {
      assert.equal(qr.isDark(i, 6), i % 2 === 0, `horizontal timing ${i}`);
      assert.equal(qr.isDark(6, i), i % 2 === 0, `vertical timing ${i}`);
    }
    assert.equal(qr.isDark(8, qr.size - 8), true, "dark module");
    const { first, second } = readFormatInfo(qr);
    assert.equal(first, second, "both format copies agree");
    assert.equal(first, bchFormat((LEVEL_FORMAT_BITS[level] << 3) | qr.mask));
    if (qr.version >= 7) {
      let bits = 0;
      for (let i = 0; i < 18; i += 1) {
        bits |= (qr.isDark(qr.size - 11 + (i % 3), Math.floor(i / 3)) ? 1 : 0) << i;
      }
      assert.equal(bits >>> 12, qr.version, "version information");
    }
    for (const x of alignmentPatternPositions(qr.version)) {
      for (const y of alignmentPatternPositions(qr.version)) {
        const nearFinder = (x < 9 && y < 9) || (x > qr.size - 10 && y < 9) || (x < 9 && y > qr.size - 10);
        if (!nearFinder) {
          assert.equal(qr.isDark(x, y), true, `alignment center ${x},${y}`);
          assert.equal(qr.isDark(x + 1, y), false, `alignment ring ${x + 1},${y}`);
        }
      }
    }
  }
});

test("capacity tables follow ISO/IEC 18004", () => {
  assert.equal(rawDataModuleCount(1), 208);
  assert.equal(rawDataModuleCount(40), 29648);
  assert.deepEqual(
    [dataCodewordCapacity(1, "L"), dataCodewordCapacity(1, "M"), dataCodewordCapacity(1, "Q"), dataCodewordCapacity(1, "H")],
    [19, 16, 13, 9],
  );
  assert.deepEqual(
    [dataCodewordCapacity(40, "L"), dataCodewordCapacity(40, "M"), dataCodewordCapacity(40, "Q"), dataCodewordCapacity(40, "H")],
    [2956, 2334, 1666, 1276],
  );
  assert.deepEqual(alignmentPatternPositions(1), []);
  assert.deepEqual(alignmentPatternPositions(7), [6, 22, 38]);
  assert.deepEqual(alignmentPatternPositions(32), [6, 34, 60, 86, 112, 138]);
});

test("the smallest fitting version is selected and limits are enforced", () => {
  // Version 1-M holds 14 bytes in byte mode.
  assert.equal(encodeQr("x".repeat(14), { level: "M" }).version, 1);
  assert.equal(encodeQr("x".repeat(15), { level: "M" }).version, 2);
  assert.equal(encodeQr("x".repeat(2953), { level: "L" }).version, 40);
  assert.throws(() => encodeQr("x".repeat(2954), { level: "L" }), (error) =>
    error instanceof QrEncodeError && error.code === "data_too_long");
  assert.throws(() => encodeQr("x".repeat(40), { level: "M", maxVersion: 2 }), { code: "data_too_long" });
});

test("invalid options are rejected", () => {
  assert.throws(() => encodeQr(42), { code: "invalid_option" });
  assert.throws(() => encodeQr("a", { level: "X" }), { code: "invalid_option" });
  assert.throws(() => encodeQr("a", { mask: 8 }), { code: "invalid_option" });
  assert.throws(() => encodeQr("a", { minVersion: 0 }), { code: "invalid_option" });
  assert.throws(() => encodeQr("a", { minVersion: 5, maxVersion: 4 }), { code: "invalid_option" });
  const qr = encodeQr("a");
  assert.throws(() => qrSvgPath(qr, -1), { code: "invalid_option" });
  assert.throws(() => qrToSvg(qr, { dark: "url(javascript:alert(1))" }), { code: "invalid_option" });
});

test("a fixed mask is honored", () => {
  for (let mask = 0; mask < 8; mask += 1) {
    const qr = encodeQr("IrohaConnect", { mask });
    assert.equal(qr.mask, mask);
    assert.equal(readFormatInfo(qr).first, bchFormat((LEVEL_FORMAT_BITS.M << 3) | mask));
  }
});

test("SVG output covers every dark module and escapes the title", () => {
  const qr = encodeQr("https://taira.sora.org");
  let dark = 0;
  for (const module of qr.modules) dark += module;
  const path = qrSvgPath(qr, 4);
  const covered = [...path.matchAll(/h(\d+)v1/gu)].reduce((sum, match) => sum + Number(match[1]), 0);
  assert.equal(covered, dark);
  assert.match(path, /^M\d+ \d+h\d+v1h-\d+z/u);
  const svg = qrToSvg(qr, { title: `<Pairing & "code">` });
  assert.match(svg, /viewBox="0 0 33 33"/u);
  assert.match(svg, /shape-rendering="crispEdges"/u);
  assert.ok(svg.includes("<title>&#60;Pairing &#38; &#34;code&#34;&#62;</title>"));
  assert.ok(!svg.includes("<Pairing"));
  assert.ok(!qrToSvg(qr).includes("aria-label"));
});

"use strict";

import test from "node:test";
import assert from "node:assert/strict";
import {
  readdirSync,
  readFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { AccountAddress } from "../src/address.js";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const I105_ALPHABET =
  "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz" +
  "ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ";
const SAMPLE_ACCOUNT_PATTERN = new RegExp(
  `sorauﾛ1[${I105_ALPHABET}]+`,
  "gu",
);

function shippedDocumentationFiles() {
  return [
    join(ROOT, "README.md"),
    join(ROOT, "recipes", "README.md"),
    ...readdirSync(join(ROOT, "recipes"), { withFileTypes: true })
      .filter((entry) => entry.isFile() && entry.name.endsWith(".mjs"))
      .map((entry) => join(ROOT, "recipes", entry.name)),
  ];
}

test("source documentation uses canonical, curve-valid I105 account samples", () => {
  const occurrences = [];
  for (const file of shippedDocumentationFiles()) {
    const source = readFileSync(file, "utf8");
    for (const match of source.matchAll(SAMPLE_ACCOUNT_PATTERN)) {
      occurrences.push({ file, literal: match[0] });
    }
  }

  assert.ok(
    occurrences.length >= 50,
    `account-literal scan unexpectedly found only ${occurrences.length} samples`,
  );
  for (const { file, literal } of occurrences) {
    let parsed;
    assert.doesNotThrow(
      () => {
        parsed = AccountAddress.parseEncoded(literal);
      },
      `${file} contains an invalid account sample: ${literal}`,
    );
    assert.equal(
      parsed.address.toI105(parsed.chainDiscriminant),
      literal,
      `${file} contains a non-canonical account sample: ${literal}`,
    );
  }
});

test("the source-checkout batching recipe runs offline end to end", () => {
  const result = spawnSync(process.execPath, [join(ROOT, "recipes", "batching.mjs")], {
    cwd: ROOT,
    encoding: "utf8",
    env: { ...process.env },
    timeout: 120_000,
  });

  assert.equal(result.signal, null, result.stderr || result.stdout);
  assert.equal(result.status, 0, result.stderr || result.stdout);
  for (const marker of [
    "=== Mint ===",
    "=== Transfer ===",
    "=== Burn ===",
    "Manual batch hash:",
    "Mint + transfer helper hash:",
    "Register + mint + transfer hash:",
  ]) {
    assert.match(result.stdout, new RegExp(marker.replace(/[+]/gu, "\\+"), "u"));
  }
  assert.equal(result.stderr, "");
});

test("live recipes reject ambiguous security flags before I/O", () => {
  const iterator = spawnSync(
    process.execPath,
    [join(ROOT, "recipes", "assets_iterators.mjs")],
    {
      cwd: ROOT,
      encoding: "utf8",
      env: {
        ...process.env,
        TORII_REQUIRE_PERMISSIONS: "yes",
      },
      timeout: 30_000,
    },
  );
  assert.equal(iterator.status, 1, iterator.stdout);
  assert.match(
    iterator.stderr,
    /TORII_REQUIRE_PERMISSIONS must be exactly 0 or 1/u,
  );

  const insecure = spawnSync(
    process.execPath,
    [join(ROOT, "recipes", "assets_iterators.mjs")],
    {
      cwd: ROOT,
      encoding: "utf8",
      env: {
        ...process.env,
        TORII_REQUIRE_PERMISSIONS: "0",
        TORII_ALLOW_INSECURE: "yes",
      },
      timeout: 30_000,
    },
  );
  assert.equal(insecure.status, 1, insecure.stdout);
  assert.match(
    insecure.stderr,
    /TORII_ALLOW_INSECURE must be exactly 0 or 1/u,
  );
  assert.doesNotMatch(insecure.stderr, /ECONNREFUSED|fetch failed/u);
});

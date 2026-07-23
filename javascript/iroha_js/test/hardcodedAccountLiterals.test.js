import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { AccountAddress } from "../src/address.js";

const PACKAGE_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SKIPPED_DIRECTORIES = new Set([
  "build",
  "coverage",
  "dist",
  "native",
  "node_modules",
]);
const TEXT_EXTENSIONS = new Set([".cjs", ".js", ".json", ".md", ".mjs", ".ts"]);
const DEFAULT_I105_PREFIX = ["sora", "u"].join("");
const DEFAULT_I105_LITERAL_PATTERN = new RegExp(
  `${DEFAULT_I105_PREFIX}[\\p{L}\\p{N}\\uFF61-\\uFF9F]{40,}`,
  "gu",
);

function* packageTextFiles(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    if (entry.isDirectory() && SKIPPED_DIRECTORIES.has(entry.name)) {
      continue;
    }
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      yield* packageTextFiles(entryPath);
    } else if (entry.isFile() && TEXT_EXTENSIONS.has(path.extname(entry.name))) {
      yield entryPath;
    }
  }
}

test("hard-coded default-chain I105 account literals are strictly valid and canonical", () => {
  let checked = 0;
  for (const filePath of packageTextFiles(PACKAGE_ROOT)) {
    const source = fs.readFileSync(filePath, "utf8");
    for (const match of source.matchAll(DEFAULT_I105_LITERAL_PATTERN)) {
      const literal = match[0];
      const line = source.slice(0, match.index).split("\n").length;
      const location = `${path.relative(PACKAGE_ROOT, filePath)}:${line}`;
      let address;
      try {
        address = AccountAddress.fromI105(literal);
      } catch (error) {
        assert.fail(`${location} contains an invalid positive I105 literal: ${error.message}`);
      }
      assert.equal(
        address.toI105(),
        literal,
        `${location} must use the exact canonical I105 rendering`,
      );
      checked += 1;
    }
  }
  assert.ok(checked > 0, "expected at least one hard-coded I105 account literal");
});

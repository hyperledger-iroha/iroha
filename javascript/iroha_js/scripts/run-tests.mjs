#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const PACKAGE_ROOT = resolve(dirname(SCRIPT_PATH), "..");
const TEST_SUFFIXES = [".test.js", ".test.mjs"];

function compareText(left, right) {
  if (left < right) {
    return -1;
  }
  if (left > right) {
    return 1;
  }
  return 0;
}

export function collectTestFiles(testRoot = join(PACKAGE_ROOT, "test")) {
  const files = [];

  function visit(directory) {
    const entries = readdirSync(directory, { withFileTypes: true }).sort((left, right) =>
      compareText(left.name, right.name),
    );
    for (const entry of entries) {
      const entryPath = join(directory, entry.name);
      if (entry.isDirectory()) {
        visit(entryPath);
      } else if (
        entry.isFile() &&
        TEST_SUFFIXES.some((suffix) => entry.name.endsWith(suffix))
      ) {
        files.push(entryPath);
      }
    }
  }

  visit(resolve(testRoot));
  return files;
}

export function runTests({
  packageRoot = PACKAGE_ROOT,
  testRoot,
  testArgs = process.argv.slice(2),
  spawn = spawnSync,
} = {}) {
  const resolvedPackageRoot = resolve(packageRoot);
  const resolvedTestRoot = resolve(testRoot ?? join(resolvedPackageRoot, "test"));
  const testFiles = collectTestFiles(resolvedTestRoot);
  if (testFiles.length === 0) {
    throw new Error(`no JavaScript test files found under ${resolvedTestRoot}`);
  }

  const result = spawn(
    process.execPath,
    [
      "--test",
      ...testArgs,
      ...testFiles.map((file) => relative(resolvedPackageRoot, file)),
    ],
    {
      cwd: resolvedPackageRoot,
      stdio: "inherit",
    },
  );
  if (result.error) {
    throw result.error;
  }
  return result.status ?? 1;
}

const invokedAsMain =
  process.argv[1] !== undefined &&
  pathToFileURL(resolve(process.argv[1])).href === import.meta.url;
if (invokedAsMain) {
  process.exitCode = runTests();
}

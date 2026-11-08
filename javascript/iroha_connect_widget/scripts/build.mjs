#!/usr/bin/env node
// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Build the self-contained browser bundles in `dist/`:
 *
 * - `iroha-connect.js` registers `<iroha-connect>` and `<iroha-sakura>`; the
 *   Connect SDK (crypto, Torii client) is split into a chunk that loads the
 *   first time a user starts pairing.
 * - `iroha-sakura.js` registers `<iroha-sakura>` only.
 *
 * Both are ES modules for `<script type="module">` on any page.
 */

import { build } from "esbuild";
import { rmSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const OUT_DIR = path.join(ROOT, "dist");

rmSync(OUT_DIR, { recursive: true, force: true });

const result = await build({
  absWorkingDir: ROOT,
  entryPoints: {
    "iroha-connect": "src/index.js",
    "iroha-sakura": "src/sakura-element.js",
  },
  outdir: OUT_DIR,
  bundle: true,
  splitting: true,
  format: "esm",
  platform: "browser",
  conditions: ["browser"],
  target: ["es2022"],
  minify: true,
  sourcemap: true,
  legalComments: "linked",
  chunkNames: "chunks/[name]-[hash]",
  metafile: true,
  logLevel: "warning",
});

const nodeBuiltins = Object.keys(result.metafile.inputs).filter((input) => input.startsWith("node:"));
if (nodeBuiltins.length > 0) {
  throw new Error(`browser bundle pulled in Node built-ins: ${nodeBuiltins.join(", ")}`);
}

for (const output of Object.keys(result.metafile.outputs).filter((file) => file.endsWith(".js")).sort()) {
  const bytes = statSync(path.join(ROOT, output)).size;
  console.log(`${path.relative(ROOT, path.join(ROOT, output)).padEnd(44)} ${(bytes / 1024).toFixed(1).padStart(8)} KiB`);
}

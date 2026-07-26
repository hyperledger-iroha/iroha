#!/usr/bin/env node
/**
 * Refresh the shared AXT descriptor fixture using the native binding to compute
 * canonical Norito bytes and the Poseidon binding.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { buildAxtDescriptor } from "../src/axt.js";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..", "..");
const fixturePath = path.resolve(
  repoRoot,
  "crates/iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json",
);

const input = {
  dsids: [7, 1],
  touches: [
    { dsid: 1, read: ["payments/", "orders/", "orders/"], write: ["ledger/"] },
    { dsid: 7, read: ["reports/"], write: ["audits/", "aggregates/", "audits/"] },
  ],
  touchManifest: [
    {
      dsid: 1,
      manifest: { read: ["orders/root"], write: ["ledger/settlement"] },
    },
    {
      dsid: 7,
      manifest: {
        read: ["reports/monthly", "reports/monthly"],
        write: ["audits/summary", "aggregates/monthly", "audits/summary"],
      },
    },
  ],
};

const result = buildAxtDescriptor(input);
if (!result.native || !result.bindingHex || !result.descriptorBytes) {
  console.error(
    "[axt] native binding is required to refresh fixtures; run `npm run build:native` first.",
  );
  process.exit(1);
}

const payload = {
  descriptor: result.descriptor,
  descriptor_hex: result.descriptorBytes.toString("hex"),
  touch_manifest: result.touchManifest,
  binding_hex: result.bindingHex,
};
fs.writeFileSync(fixturePath, `${JSON.stringify(payload, null, 2)}\n`);
console.log(`[axt] refreshed fixture at ${fixturePath}`);

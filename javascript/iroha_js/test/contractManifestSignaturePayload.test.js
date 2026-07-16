import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { verifyEd25519 } from "../src/crypto.browser.js";
import {
  noritoEncodeContractManifestSignaturePayload,
  validateNoritoFrame,
} from "../src/norito.js";

const FIXTURE_URL = new URL("./fixtures/contract_manifest_v1.json", import.meta.url);

test("manifest signature payload matches and verifies the current Rust fixture", async () => {
  const fixture = JSON.parse(await readFile(FIXTURE_URL, "utf8"));
  const frame = noritoEncodeContractManifestSignaturePayload(fixture.manifest);
  const decoded = validateNoritoFrame(frame, {
    expectedSchemaHash: Buffer.from(
      "b4bb42540d44c468ed44d5f94c59b007",
      "hex",
    ),
    expectedPaddingLength: 0,
  });

  assert.equal(decoded.flags, 0x02);
  assert.equal(
    decoded.payload.toString("hex"),
    fixture.manifest_compact_hex.slice(0, -4),
  );

  const signerLiteral = fixture.signed_provenance.signer;
  assert.match(signerLiteral, /^ed0120[0-9A-F]{64}$/u);
  const publicKey = Buffer.from(signerLiteral.slice(6), "hex");
  const signature = Buffer.from(fixture.signed_provenance.signature, "hex");
  assert.equal(verifyEd25519(frame, signature, publicKey), true);
});

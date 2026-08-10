import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
  AUTHENTICATED_BLOCK_PROOFS_MAX_FINALITY_PROOF_BYTES_V1,
  AUTHENTICATED_BLOCK_PROOFS_MAX_PROOF_BYTES_V1,
  AUTHENTICATED_BLOCK_PROOFS_VERSION_V1,
  verifyAuthenticatedBlockProofsV1,
} from "../src/authenticatedBlockProofs.js";
import * as browser from "../src/browser.js";
import * as root from "../src/index.js";

function minimallyShapedInput(overrides = {}) {
  const context = Buffer.alloc(32);
  context[31] = 1;
  const expectedEntryHash = Buffer.alloc(32);
  expectedEntryHash[31] = 1;
  return {
    version: 1,
    networkId: "a5".repeat(32),
    trustedContextId: context,
    expectedEntryHash,
    finalityProofNorito: Buffer.of(1),
    executedBlockWire: Buffer.of(1),
    blockProofsNorito: Buffer.of(1),
    ...overrides,
  };
}

test("authenticated BlockProofs exports keep root and browser manifests aligned", () => {
  assert.equal(AUTHENTICATED_BLOCK_PROOFS_VERSION_V1, 1);
  assert.equal(AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1, 32 * 1024 * 1024);
  assert.equal(AUTHENTICATED_BLOCK_PROOFS_MAX_FINALITY_PROOF_BYTES_V1, 9 * 1024 * 1024);
  assert.equal(AUTHENTICATED_BLOCK_PROOFS_MAX_PROOF_BYTES_V1, 16 * 1024 * 1024);
  for (const runtime of [root, browser]) {
    assert.equal(typeof runtime.verifyAuthenticatedBlockProofsV1, "function");
    assert.equal(runtime.AUTHENTICATED_BLOCK_PROOFS_VERSION_V1, 1);
  }
});

test("browser authenticated verifier fails closed without digest-pinned Rust WASM", async () => {
  await assert.rejects(
    browser.verifyAuthenticatedBlockProofsV1(minimallyShapedInput()),
    (error) => {
      assert.equal(error.code, "ERR_IROHA_AUTHENTICATED_BLOCK_PROOFS_UNAVAILABLE");
      assert.match(error.message, /no digest-pinned browser finality-verifier WASM/u);
      return true;
    },
  );
});

test("node wrapper rejects unknown fields, wrong versions, and noncanonical networks before native load", async () => {
  await assert.rejects(
    verifyAuthenticatedBlockProofsV1(minimallyShapedInput({ surprise: true })),
    /unknown field surprise/u,
  );
  await assert.rejects(
    verifyAuthenticatedBlockProofsV1(minimallyShapedInput({ version: 2 })),
    /version must be 1/u,
  );
  await assert.rejects(
    verifyAuthenticatedBlockProofsV1(minimallyShapedInput({ networkId: " bad " })),
    /networkId is not canonical/u,
  );
  await assert.rejects(
    verifyAuthenticatedBlockProofsV1(minimallyShapedInput({ networkId: "a4".repeat(32) })),
    /networkId must carry the Iroha hash marker bit/u,
  );
});

test("node wrapper snapshots exact enumerable data fields without invoking accessors", async () => {
  const accessorInput = minimallyShapedInput();
  let accessorInvoked = false;
  Object.defineProperty(accessorInput, "finalityProofNorito", {
    enumerable: true,
    get() {
      accessorInvoked = true;
      return Buffer.of(1);
    },
  });
  await assert.rejects(
    verifyAuthenticatedBlockProofsV1(accessorInput),
    /finalityProofNorito must be an enumerable data property/u,
  );
  assert.equal(accessorInvoked, false);

  const symbolInput = minimallyShapedInput();
  symbolInput[Symbol("forged")] = true;
  await assert.rejects(
    verifyAuthenticatedBlockProofsV1(symbolInput),
    /contains unknown field Symbol\(forged\)/u,
  );

  const missingInput = minimallyShapedInput();
  delete missingInput.expectedEntryHash;
  await assert.rejects(
    verifyAuthenticatedBlockProofsV1(missingInput),
    /missing required field expectedEntryHash/u,
  );
});

test("node wrapper enforces the marked exact context id and closed archive bounds", async () => {
  await assert.rejects(
    verifyAuthenticatedBlockProofsV1(
      minimallyShapedInput({ trustedContextId: Buffer.alloc(31) }),
    ),
    /exactly 32 bytes/u,
  );
  await assert.rejects(
    verifyAuthenticatedBlockProofsV1(
      minimallyShapedInput({ trustedContextId: Buffer.alloc(32) }),
    ),
    /marker bit/u,
  );
  await assert.rejects(
    verifyAuthenticatedBlockProofsV1(
      minimallyShapedInput({ expectedEntryHash: Buffer.alloc(31) }),
    ),
    /expectedEntryHash must contain exactly 32 bytes/u,
  );
  await assert.rejects(
    verifyAuthenticatedBlockProofsV1(
      minimallyShapedInput({ expectedEntryHash: Buffer.alloc(32) }),
    ),
    /expectedEntryHash must carry the Iroha hash marker bit/u,
  );
  await assert.rejects(
    verifyAuthenticatedBlockProofsV1(
      minimallyShapedInput({ finalityProofNorito: Buffer.alloc(0) }),
    ),
    /must contain 1\.\.9437184 bytes/u,
  );
  await assert.rejects(
    verifyAuthenticatedBlockProofsV1(
      minimallyShapedInput({
        previousFinalityProofNorito: Buffer.alloc(
          AUTHENTICATED_BLOCK_PROOFS_MAX_FINALITY_PROOF_BYTES_V1 + 1,
        ),
      }),
    ),
    /previousFinalityProofNorito must contain/u,
  );
});

test("node wrapper rejects shared memory at every authenticated byte boundary", async (t) => {
  if (typeof SharedArrayBuffer !== "function") {
    t.skip("SharedArrayBuffer is unavailable");
    return;
  }
  for (const field of [
    "trustedContextId",
    "expectedEntryHash",
    "previousFinalityProofNorito",
    "finalityProofNorito",
    "executedBlockWire",
    "blockProofsNorito",
  ]) {
    const fixedHashField = field === "trustedContextId" || field === "expectedEntryHash";
    const bytes = new Uint8Array(new SharedArrayBuffer(fixedHashField ? 32 : 1));
    if (fixedHashField) bytes[31] = 1;
    await assert.rejects(
      verifyAuthenticatedBlockProofsV1(minimallyShapedInput({ [field]: bytes })),
      new RegExp(`${field} must not be backed by SharedArrayBuffer`, "u"),
    );
  }
});

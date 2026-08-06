import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { runInNewContext } from "node:vm";

import {
  instantiateIvmArtifactAdmissionWasm,
  verifyIvmContractArtifactAdmission,
} from "../src/ivmArtifactAdmissionWasm.js";
import {
  createStaticArtifactAdmissionVerifier,
  staticArtifactAdmissionWasm,
} from "./helpers/artifactAdmissionWasm.js";
import { parseStrictLosslessIntegerJson } from "../src/strictLosslessJson.js";

const CURRENT_ARTIFACT_FIXTURE = parseStrictLosslessIntegerJson(
  readFileSync(
    new URL("./fixtures/current_rust_contract_artifact.json", import.meta.url),
    "utf8",
  ),
  "current Rust contract artifact fixture",
);
// The strict fixture parser intentionally builds null-prototype records, while
// the WASM boundary returns ordinary JSON objects. Keep that prototype choice
// out of this wire-data comparison.
const CURRENT_ARTIFACT_MANIFEST = structuredClone(
  CURRENT_ARTIFACT_FIXTURE.manifest,
);

function successfulOutput() {
  return {
    ok: true,
    code_hash_hex: CURRENT_ARTIFACT_FIXTURE.artifact_semantics.code_hash_hex,
    abi_hash_hex: CURRENT_ARTIFACT_FIXTURE.artifact_semantics.abi_hash_hex,
    header_len: CURRENT_ARTIFACT_FIXTURE.artifact_semantics.header_len,
    code_offset: CURRENT_ARTIFACT_FIXTURE.artifact_semantics.code_offset,
    entrypoint_count: CURRENT_ARTIFACT_FIXTURE.artifact_semantics.entrypoint_count,
    manifest: CURRENT_ARTIFACT_MANIFEST,
  };
}

test("raw artifact-admission WASM is digest anchored and returns bounded typed output", async () => {
  const wasmBytes = staticArtifactAdmissionWasm(successfulOutput());
  const digest = createHash("sha256").update(wasmBytes).digest("hex");
  await assert.rejects(
    instantiateIvmArtifactAdmissionWasm({
      wasmBytes,
      expectedSha256Hex: "00".repeat(32),
    }),
    /SHA-256 mismatch/u,
  );

  const verifier = await instantiateIvmArtifactAdmissionWasm({
    wasmBytes,
    expectedSha256Hex: digest,
  });
  const artifact = Buffer.from(
    CURRENT_ARTIFACT_FIXTURE.artifact_base64,
    "base64",
  );
  const result = verifyIvmContractArtifactAdmission(verifier, artifact);
  assert.deepEqual(result, {
    ok: true,
    codeHashHex: CURRENT_ARTIFACT_FIXTURE.artifact_semantics.code_hash_hex,
    abiHashHex: CURRENT_ARTIFACT_FIXTURE.artifact_semantics.abi_hash_hex,
    headerLength: CURRENT_ARTIFACT_FIXTURE.artifact_semantics.header_len,
    codeOffset: CURRENT_ARTIFACT_FIXTURE.artifact_semantics.code_offset,
    entrypointCount: CURRENT_ARTIFACT_FIXTURE.artifact_semantics.entrypoint_count,
    manifest: CURRENT_ARTIFACT_MANIFEST,
  });
  assert.equal(Object.isFrozen(result), true);
  assert.equal(Object.isFrozen(result.manifest), true);
  assert.throws(
    () =>
      verifyIvmContractArtifactAdmission(
        { verifierSha256Hex: digest, verify: () => result },
        artifact,
      ),
    /must come from instantiateIvmArtifactAdmissionWasm/u,
  );
});

test("raw artifact-admission WASM preserves shared-policy rejection details", async () => {
  const verifier = await createStaticArtifactAdmissionVerifier({
    ok: false,
    error: "invalid contract artifact: disallowed syscall 0xfe0000 at pc 0",
  });
  const result = verifier.verify(Uint8Array.of(1));
  assert.deepEqual(result, {
    ok: false,
    error: "invalid contract artifact: disallowed syscall 0xfe0000 at pc 0",
  });
});

test("raw artifact-admission WASM rejects unmarked Iroha hashes", async () => {
  for (const field of ["code_hash_hex", "abi_hash_hex"]) {
    const output = successfulOutput();
    const finalByte = Number.parseInt(output[field].slice(-2), 16);
    assert.equal(finalByte & 1, 1);
    output[field] = `${output[field].slice(0, -2)}${(finalByte ^ 1)
      .toString(16)
      .padStart(2, "0")}`;
    const verifier = await createStaticArtifactAdmissionVerifier(output);
    assert.throws(
      () => verifier.verify(Uint8Array.of(1)),
      new RegExp(`artifact admission ${field} must carry the Iroha Hash marker bit`, "u"),
    );
  }
});

test("raw artifact-admission WASM copies cross-realm bytes and rejects shared memory", async () => {
  const wasmBytes = staticArtifactAdmissionWasm(successfulOutput());
  const foreignBuffer = runInNewContext(`new ArrayBuffer(${wasmBytes.length})`);
  new Uint8Array(foreignBuffer).set(wasmBytes);
  const expectedSha256Hex = createHash("sha256")
    .update(wasmBytes)
    .digest("hex");
  const verifier = await instantiateIvmArtifactAdmissionWasm({
    wasmBytes: foreignBuffer,
    expectedSha256Hex,
  });
  const sharedArtifact = new SharedArrayBuffer(1);
  assert.throws(
    () => verifier.verify(sharedArtifact),
    /must not be backed by SharedArrayBuffer/u,
  );
  const sharedWasm = new SharedArrayBuffer(wasmBytes.length);
  new Uint8Array(sharedWasm).set(wasmBytes);
  await assert.rejects(
    instantiateIvmArtifactAdmissionWasm({
      wasmBytes: sharedWasm,
      expectedSha256Hex,
    }),
    /must not be backed by SharedArrayBuffer/u,
  );
});

const realWasmPath =
  process.env.IROHA_IVM_ARTIFACT_ADMISSION_WASM ??
  fileURLToPath(
    new URL(
      "../../../target/wasm32-unknown-unknown/release/ivm_artifact_admission.wasm",
      import.meta.url,
    ),
  );
const hasRealWasm = existsSync(realWasmPath);

test(
  "exact shared WASM rejects a host-private SYSTEM syscall",
  { skip: hasRealWasm ? false : `shared WASM not built at ${realWasmPath}` },
  async () => {
    const wasmBytes = readFileSync(realWasmPath);
    const expectedSha256Hex = createHash("sha256")
      .update(wasmBytes)
      .digest("hex");
    const verifier = await instantiateIvmArtifactAdmissionWasm({
      wasmBytes,
      expectedSha256Hex,
    });
    const artifact = Buffer.from(
      CURRENT_ARTIFACT_FIXTURE.artifact_base64,
      "base64",
    );
    const accepted = verifier.verify(artifact);
    assert.equal(accepted.ok, true);
    assert.equal(
      accepted.codeHashHex,
      CURRENT_ARTIFACT_FIXTURE.artifact_semantics.code_hash_hex,
    );

    const forbidden = Buffer.from(artifact);
    const codeOffset = CURRENT_ARTIFACT_FIXTURE.artifact_semantics.code_offset;
    forbidden.set([0x00, 0x00, 0xfe, 0x62], codeOffset);
    const rejected = verifier.verify(forbidden);
    assert.equal(rejected.ok, false);
    assert.match(rejected.error, /disallowed syscall 0xfe0000/u);
  },
);

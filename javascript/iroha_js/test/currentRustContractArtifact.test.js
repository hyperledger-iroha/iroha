import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { verifyCompiledContractArtifact } from "../src/kotodamaCompiler/normalize.js";
import { parseStrictLosslessIntegerJson } from "../src/strictLosslessJson.js";

const TEST_DIRECTORY = path.dirname(fileURLToPath(import.meta.url));
const REPOSITORY_ROOT = path.resolve(TEST_DIRECTORY, "../../..");
const FIXTURE_DIRECTORY = path.join(TEST_DIRECTORY, "fixtures");
const FIXTURE = parseStrictLosslessIntegerJson(
  readFileSync(
    path.join(FIXTURE_DIRECTORY, "current_rust_contract_artifact.json"),
    "utf8",
  ),
  "current Rust contract artifact fixture",
);

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function gitBlobId(bytes) {
  const header = Buffer.from(`blob ${bytes.length}\0`, "utf8");
  return createHash("sha1").update(header).update(bytes).digest("hex");
}

function fixtureArtifact() {
  const canonical = Buffer.from(FIXTURE.artifact_base64, "base64");
  assert.equal(canonical.toString("base64"), FIXTURE.artifact_base64);
  return Uint8Array.from(canonical);
}

test("real current compiler artifact is source-bound and passes the browser structural boundary", () => {
  const sourceBindings = [
    [
      "javascript/iroha_js/test/fixtures/current_rust_contract_artifact.ko",
      "contract_source_git_blob",
    ],
    [
      "scripts/regenerate_current_rust_contract_artifact.py",
      "artifact_generator_git_blob",
    ],
  ];
  for (const [relativePath, fixtureField] of sourceBindings) {
    assert.equal(
      gitBlobId(readFileSync(path.join(REPOSITORY_ROOT, relativePath))),
      FIXTURE.source_provenance[fixtureField],
      `${relativePath} changed; regenerate this exact-current-source parity fixture`,
    );
  }

  const artifact = fixtureArtifact();
  assert.equal(artifact.length, FIXTURE.artifact_length);
  assert.equal(sha256(artifact), FIXTURE.artifact_sha256);
  const verified = verifyCompiledContractArtifact(
    artifact,
    FIXTURE.manifest,
    FIXTURE.artifact_semantics.code_hash_hex,
    FIXTURE.artifact_semantics.abi_hash_hex,
  );
  assert.equal(verified.codeHashHex, FIXTURE.artifact_semantics.code_hash_hex);
  assert.equal(verified.abiHashHex, FIXTURE.artifact_semantics.abi_hash_hex);
});

test("canonical provenance is platform-independent and contains no build-machine identity", () => {
  assert.equal(FIXTURE.fixture_version, 2);
  assert.deepEqual(Object.keys(FIXTURE.source_provenance).sort(), [
    "artifact_generator_git_blob",
    "closure_algorithm",
    "closure_sha256",
    "contract_source_git_blob",
    "file_count",
    "scope",
  ]);
  assert.equal(
    FIXTURE.source_provenance.scope,
    "semantic-worktree-source-closure-v2",
  );
  assert.equal(
    FIXTURE.source_provenance.closure_algorithm,
    "sha256-framed-present-path-and-bytes-v2",
  );
  assert.match(FIXTURE.source_provenance.closure_sha256, /^[0-9a-f]{64}$/u);
  assert.ok(FIXTURE.source_provenance.file_count > 0);
  assert.equal("generation_provenance" in FIXTURE, false);
  const canonical = JSON.stringify(FIXTURE);
  for (const forbidden of [
    "koto_sha256",
    "rustc_sha256",
    "ivm_rlib_sha256",
    "rust_dependency_closure_sha256",
  ]) {
    assert.equal(canonical.includes(forbidden), false);
  }
});

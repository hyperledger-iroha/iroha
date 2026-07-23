import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { computeIvmArtifactHashes } from "../src/ivmArtifact.js";
import { verifyCompiledContractArtifact } from "../src/kotodamaCompiler/normalize.js";

const TEST_DIRECTORY = path.dirname(fileURLToPath(import.meta.url));
const REPOSITORY_ROOT = path.resolve(TEST_DIRECTORY, "../../..");
const FIXTURE_DIRECTORY = path.join(TEST_DIRECTORY, "fixtures");
const FIXTURE = JSON.parse(
  readFileSync(
    path.join(FIXTURE_DIRECTORY, "current_rust_contract_artifact.json"),
    "utf8",
  ),
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

function canonicalHashLiteral(hex) {
  const body = hex.toUpperCase();
  let crc = 0xffff;
  const processByte = (byte) => {
    crc ^= (byte & 0xff) << 8;
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 0x8000) !== 0
          ? ((crc << 1) ^ 0x1021) & 0xffff
          : (crc << 1) & 0xffff;
    }
  };
  for (const byte of Buffer.from(`hash:${body}`, "utf8")) processByte(byte);
  return `hash:${body}#${crc.toString(16).toUpperCase().padStart(4, "0")}`;
}

function locatePinnedIvmRlib() {
  const dependencyDirectory = path.join(REPOSITORY_ROOT, "target/release/deps");
  const candidates = readdirSync(dependencyDirectory)
    .filter((name) => /^libivm-[0-9a-f]+\.rlib$/u.test(name))
    .map((name) => path.join(dependencyDirectory, name))
    .filter(
      (candidate) =>
        sha256(readFileSync(candidate)) === FIXTURE.generation_provenance.ivm_rlib_sha256,
    );
  assert.equal(
    candidates.length,
    1,
    "the exact pinned current-source IVM release rlib must be available",
  );
  return candidates[0];
}

function compileRustOracle(directory) {
  const executable = path.join(directory, "verify-current-ivm-artifact");
  execFileSync(
    "rustc",
    [
      "--edition=2024",
      path.join(FIXTURE_DIRECTORY, "verify_current_rust_contract_artifact.rs"),
      "-L",
      `dependency=${path.join(REPOSITORY_ROOT, "target/release/deps")}`,
      "--extern",
      `ivm=${locatePinnedIvmRlib()}`,
      "-o",
      executable,
    ],
    { cwd: REPOSITORY_ROOT, stdio: "pipe" },
  );
  return executable;
}

test("real current compiler artifact is source-bound and passes the browser structural boundary", () => {
  const sourceBindings = [
    ["crates/ivm/src/contract_artifact.rs", "contract_artifact_rs_git_blob"],
    ["crates/ivm_abi/src/syscalls.rs", "ivm_syscalls_rs_git_blob"],
    ["crates/kotodama_lang/src/compiler.rs", "kotodama_compiler_rs_git_blob"],
    ["Cargo.lock", "cargo_lock_git_blob"],
  ];
  for (const [relativePath, fixtureField] of sourceBindings) {
    assert.equal(
      gitBlobId(readFileSync(path.join(REPOSITORY_ROOT, relativePath))),
      FIXTURE.generation_provenance[fixtureField],
      `${relativePath} changed; regenerate this exact-current-source parity fixture`,
    );
  }

  const artifact = fixtureArtifact();
  assert.equal(artifact.length, FIXTURE.artifact_length);
  assert.equal(sha256(artifact), FIXTURE.artifact_sha256);
  const verified = verifyCompiledContractArtifact(
    artifact,
    FIXTURE.manifest,
    FIXTURE.rust_verifier.code_hash_hex,
    FIXTURE.rust_verifier.abi_hash_hex,
  );
  assert.equal(verified.codeHashHex, FIXTURE.rust_verifier.code_hash_hex);
  assert.equal(verified.abiHashHex, FIXTURE.rust_verifier.abi_hash_hex);
});

test(
  "current Rust admission oracle proves executable validation missing from the structural JS boundary",
  { skip: process.env.IROHA_JS_RUN_RUST_ARTIFACT_PARITY !== "1" },
  () => {
    const directory = mkdtempSync(path.join(tmpdir(), "iroha-js-rust-artifact-"));
    try {
      const executable = compileRustOracle(directory);
      const artifactPath = path.join(directory, "current.to");
      const artifact = fixtureArtifact();
      writeFileSync(artifactPath, artifact);

      const positive = spawnSync(executable, [artifactPath], {
        cwd: REPOSITORY_ROOT,
        encoding: "utf8",
      });
      assert.equal(positive.status, 0, positive.stderr);
      for (const [field, expected] of Object.entries(FIXTURE.rust_verifier)) {
        assert.match(positive.stdout, new RegExp(`^${field}=${expected}$`, "mu"));
      }

      const invalid = artifact.slice();
      invalid.set(
        Uint8Array.of(0x00, 0x00, 0xfe, 0x62),
        FIXTURE.rust_verifier.code_offset,
      );
      const invalidPath = path.join(directory, "invalid-opcode.to");
      writeFileSync(invalidPath, invalid);
      const invalidHash = computeIvmArtifactHashes(invalid).codeHashHex;
      const invalidManifest = JSON.parse(JSON.stringify(FIXTURE.manifest));
      invalidManifest.code_hash = canonicalHashLiteral(invalidHash);

      assert.doesNotThrow(() =>
        verifyCompiledContractArtifact(
          invalid,
          invalidManifest,
          invalidHash,
          FIXTURE.rust_verifier.abi_hash_hex,
        ));
      const negative = spawnSync(executable, [invalidPath], {
        cwd: REPOSITORY_ROOT,
        encoding: "utf8",
      });
      assert.equal(negative.status, 2);
      assert.match(negative.stderr, /invalid contract artifact/u);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  },
);

import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { build } from "esbuild";

import { findForbiddenBrowserInputs } from "../scripts/bundle-size-check.mjs";

import {
  computeIvmArtifactHashes as computeFromMain,
  IVM_ARTIFACT_MAX_BYTES as mainMaxBytes,
  IVM_PROGRAM_HEADER_LENGTH as mainHeaderLength,
} from "../dist/index.js";
import {
  computeIvmArtifactHashes as computeFromBrowser,
  IVM_ARTIFACT_MAX_BYTES as browserMaxBytes,
  IVM_PROGRAM_HEADER_LENGTH as browserHeaderLength,
} from "@iroha/iroha-js/browser";
import {
  computeIvmArtifactHashes as computeFromSubpath,
  IVM_ARTIFACT_MAX_BYTES as subpathMaxBytes,
  IVM_PROGRAM_HEADER_LENGTH as subpathHeaderLength,
} from "@iroha/iroha-js/ivm-artifact";

const ARTIFACT = Uint8Array.from([
  0x49, 0x56, 0x4d, 0x00,
  0x01, 0x01, 0x01, 0x00,
  0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
  0x01,
  ...Array(32).fill(0),
]);

test("packed entrypoints expose identical browser-safe IVM artifact identities", () => {
  assert.equal(mainHeaderLength, 49);
  assert.equal(browserHeaderLength, 49);
  assert.equal(subpathHeaderLength, 49);
  assert.equal(mainMaxBytes, 4 * 1024 * 1024);
  assert.equal(browserMaxBytes, mainMaxBytes);
  assert.equal(subpathMaxBytes, mainMaxBytes);
  const expected = {
    codeHashHex:
      "b5d6d7f7abf5989ca07b4fbee75560ab7a3dbceaafd442da66a6918e3cb147d1",
    artifactSha256Hex:
      "b004dd0c3eddd8e1c729e18ce88a2c6ab225fc21f3be1ccf55bac71d403826e6",
  };
  assert.deepEqual(computeFromMain(ARTIFACT), expected);
  assert.deepEqual(computeFromBrowser(ARTIFACT), expected);
  assert.deepEqual(computeFromSubpath(ARTIFACT), expected);
  assert.equal(globalThis.Buffer, Buffer);
});

test("packed ivm-artifact subpath bundles and compiles without ambient Node types", async () => {
  const packageRoot = fileURLToPath(new URL("..", import.meta.url));
  const tempRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "iroha-js-ivm-artifact-browser-"),
  );
  try {
    const packed = spawnSync(
      "npm",
      ["pack", "--ignore-scripts", "--json", "--pack-destination", tempRoot],
      { cwd: packageRoot, encoding: "utf8" },
    );
    assert.equal(
      packed.status,
      0,
      `npm pack failed:\n${packed.stdout}\n${packed.stderr}`,
    );
    const [packResult] = JSON.parse(packed.stdout);
    const extracted = spawnSync(
      "tar",
      ["-xzf", path.join(tempRoot, packResult.filename), "-C", tempRoot],
      { encoding: "utf8" },
    );
    assert.equal(
      extracted.status,
      0,
      `tar extraction failed:\n${extracted.stdout}\n${extracted.stderr}`,
    );
    const packagePath = path.join(
      tempRoot,
      "node_modules",
      "@iroha",
      "iroha-js",
    );
    fs.mkdirSync(path.dirname(packagePath), { recursive: true });
    fs.renameSync(path.join(tempRoot, "package"), packagePath);

    const declarations = fs.readFileSync(
      path.join(packagePath, "ivm-artifact.d.ts"),
      "utf8",
    );
    assert.doesNotMatch(
      declarations,
      /reference types=["']node|from ["'](?:node:|buffer["'])|\bBuffer\b/u,
    );
    const packedPackage = JSON.parse(
      fs.readFileSync(path.join(packagePath, "package.json"), "utf8"),
    );
    assert.equal(
      packedPackage.exports["./ivm-artifact"].types,
      "./ivm-artifact.d.ts",
    );
    assert.deepEqual(packedPackage.typesVersions["*"]["ivm-artifact"], [
      "./ivm-artifact.d.ts",
    ]);

    const entryPoint = path.join(tempRoot, "consumer.mjs");
    fs.writeFileSync(
      entryPoint,
      'export * from "@iroha/iroha-js/ivm-artifact";\n',
      "utf8",
    );
    const bundle = await build({
      entryPoints: [entryPoint],
      absWorkingDir: tempRoot,
      nodePaths: [path.join(packageRoot, "node_modules")],
      bundle: true,
      write: false,
      platform: "browser",
      target: "es2020",
      format: "esm",
      treeShaking: true,
      sourcemap: false,
      minify: true,
      metafile: true,
    });
    const inputs = Object.keys(bundle.metafile.inputs);
    assert.deepEqual(findForbiddenBrowserInputs(inputs), []);
    assert.ok(inputs.some((input) => /dist[/\\]ivmArtifact\.js$/u.test(input)));
    assert.equal(inputs.some((input) => /^node:/u.test(input)), false);
    assert.doesNotMatch(
      bundle.outputFiles[0].text,
      /(?:globalThis|window|global)\.Buffer\s*=/u,
    );
    assert.ok(
      bundle.outputFiles[0].contents.byteLength <= 12 * 1024,
      `packed ivm-artifact browser bundle is ${bundle.outputFiles[0].contents.byteLength} bytes`,
    );
    const packedModule = await import(
      `data:text/javascript;base64,${Buffer.from(bundle.outputFiles[0].contents).toString("base64")}`
    );
    assert.equal(packedModule.IVM_ARTIFACT_MAX_BYTES, mainMaxBytes);
    assert.deepEqual(packedModule.computeIvmArtifactHashes(ARTIFACT), {
      codeHashHex:
        "b5d6d7f7abf5989ca07b4fbee75560ab7a3dbceaafd442da66a6918e3cb147d1",
      artifactSha256Hex:
        "b004dd0c3eddd8e1c729e18ce88a2c6ab225fc21f3be1ccf55bac71d403826e6",
    });
    const offsetBytes = new Uint8Array(ARTIFACT.byteLength + 9);
    offsetBytes.set(ARTIFACT, 5);
    assert.deepEqual(
      packedModule.computeIvmArtifactHashes(
        new DataView(offsetBytes.buffer, 5, ARTIFACT.byteLength),
      ),
      packedModule.computeIvmArtifactHashes(ARTIFACT),
    );
    const original = packedModule.computeIvmArtifactHashes(ARTIFACT);
    const changedHeader = ARTIFACT.slice();
    changedHeader[16] ^= 0x80;
    const headerHashes = packedModule.computeIvmArtifactHashes(changedHeader);
    assert.notEqual(headerHashes.codeHashHex, original.codeHashHex);
    assert.notEqual(
      headerHashes.artifactSha256Hex,
      original.artifactSha256Hex,
    );
    const changedBody = Uint8Array.from([...ARTIFACT, 0x80]);
    const bodyHashes = packedModule.computeIvmArtifactHashes(changedBody);
    assert.notEqual(bodyHashes.codeHashHex, original.codeHashHex);
    assert.notEqual(bodyHashes.artifactSha256Hex, original.artifactSha256Hex);
    assert.throws(
      () =>
        packedModule.computeIvmArtifactHashes(
          new Uint8Array(mainMaxBytes + 1),
        ),
      /exceeds the 4194304-byte limit/,
    );
    if (typeof SharedArrayBuffer === "function") {
      assert.throws(
        () =>
          packedModule.computeIvmArtifactHashes(
            new Uint8Array(new SharedArrayBuffer(ARTIFACT.byteLength)),
          ),
        /must not be backed by SharedArrayBuffer/,
      );
    }

    fs.writeFileSync(
      path.join(tempRoot, "consumer.ts"),
      [
        "import {",
        "  computeIvmArtifactHashes,",
        "  IVM_ARTIFACT_MAX_BYTES,",
        "  IVM_PROGRAM_HEADER_LENGTH,",
        '  type IvmArtifactHashes,',
        '} from "@iroha/iroha-js/ivm-artifact";',
        "const artifact = new Uint8Array(IVM_PROGRAM_HEADER_LENGTH);",
        "const maxBytes: 4194304 = IVM_ARTIFACT_MAX_BYTES;",
        "const hashes: IvmArtifactHashes = computeIvmArtifactHashes(artifact);",
        "const fromBuffer: IvmArtifactHashes = computeIvmArtifactHashes(artifact.buffer);",
        "const fromView: IvmArtifactHashes = computeIvmArtifactHashes(new DataView(artifact.buffer));",
        "const codeHash: string = hashes.codeHashHex;",
        "const artifactHash: string = hashes.artifactSha256Hex;",
        "void fromBuffer; void fromView; void codeHash; void artifactHash; void maxBytes;",
      ].join("\n"),
      "utf8",
    );
    fs.writeFileSync(
      path.join(tempRoot, "tsconfig.json"),
      JSON.stringify(
        {
          compilerOptions: {
            strict: true,
            exactOptionalPropertyTypes: true,
            noUncheckedIndexedAccess: true,
            target: "ES2022",
            module: "NodeNext",
            moduleResolution: "NodeNext",
            lib: ["ES2022", "DOM"],
            types: [],
            noEmit: true,
          },
          files: ["consumer.ts"],
        },
        null,
        2,
      ),
      "utf8",
    );
    assert.equal(
      fs.existsSync(path.join(tempRoot, "node_modules", "@types", "node")),
      false,
      "strict-DOM proof must not resolve ambient Node declarations",
    );
    const tsc = fileURLToPath(
      new URL("../node_modules/typescript/bin/tsc", import.meta.url),
    );
    const compiled = spawnSync(process.execPath, [tsc, "-p", "tsconfig.json"], {
      cwd: tempRoot,
      encoding: "utf8",
    });
    assert.equal(
      compiled.status,
      0,
      `packed ivm-artifact strict-DOM compile failed:\n${compiled.stdout}\n${compiled.stderr}`,
    );
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
});

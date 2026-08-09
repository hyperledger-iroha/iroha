import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import test from "node:test";

const retiredSourceModule = new URL(
  "../src/ivmArtifactAdmissionWasm.js",
  import.meta.url,
);
const retiredDeclaration = new URL(
  "../ivm-artifact-admission-wasm.d.ts",
  import.meta.url,
);
const retiredTestHelper = new URL(
  "./helpers/artifactAdmissionWasm.js",
  import.meta.url,
);
const retiredSurfacePattern =
  /\b(?:IVM_ARTIFACT_ADMISSION_MAX_INPUT_BYTES|instantiateIvmArtifactAdmissionWasm|verifyIvmContractArtifactAdmission|artifactAdmissionVerifier)\b/u;

function sourceText(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("browser artifact-admission WASM is absent from the first-release package", async () => {
  assert.equal(existsSync(retiredSourceModule), false);
  assert.equal(existsSync(retiredDeclaration), false);
  assert.equal(existsSync(retiredTestHelper), false);

  const packageManifest = JSON.parse(sourceText("../package.json"));
  assert.equal(
    Object.hasOwn(
      packageManifest.exports,
      "./ivm-artifact-admission-wasm",
    ),
    false,
  );

  for (const relativePath of [
    "../src/index.js",
    "../dist/index.js",
    "../src/smartContractDeployment.js",
    "../dist/smartContractDeployment.js",
    "../smart-contract-deployment.d.ts",
  ]) {
    assert.doesNotMatch(
      sourceText(relativePath),
      retiredSurfacePattern,
      relativePath,
    );
  }

  const [sourceSdk, packageSdk] = await Promise.all([
    import("../src/index.js"),
    import("../dist/index.js"),
  ]);
  for (const surface of [sourceSdk, packageSdk]) {
    assert.equal(surface.instantiateIvmArtifactAdmissionWasm, undefined);
    assert.equal(surface.verifyIvmContractArtifactAdmission, undefined);
    assert.equal(surface.IVM_ARTIFACT_ADMISSION_MAX_INPUT_BYTES, undefined);
  }
});

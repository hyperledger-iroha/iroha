import assert from "node:assert/strict";
import test from "node:test";

import {
  buildSoraCloudHfDeployRequest,
  generateKeyPair,
} from "../src/index.js";

const CANONICAL_XOR_ASSET_DEFINITION_ID = "61CtjvNd9T3THAR65GsMVHr82Bjc";

function privateKeyHex() {
  const keypair = generateKeyPair();
  return Buffer.from(keypair.privateKey).toString("hex");
}

test("buildSoraCloudHfDeployRequest includes generated HF provenance", () => {
  const request = buildSoraCloudHfDeployRequest({
    repoId: "sentence-transformers/all-MiniLM-L6-v2",
    revision: "main",
    modelName: "all-MiniLM-L6-v2",
    serviceName: "all_minilm_l6_v2",
    apartmentName: "all_minilm_agent",
    storageClass: "warm",
    leaseTermMs: "3600000",
    leaseAssetDefinitionId: CANONICAL_XOR_ASSET_DEFINITION_ID,
    baseFeeNanos: "1",
    privateKeyHex: privateKeyHex(),
  });

  assert.equal(
    request.payload.repo_id,
    "sentence-transformers/all-MiniLM-L6-v2",
  );
  assert.equal(request.payload.service_name, "all_minilm_l6_v2");
  assert.equal(
    request.payload.lease_asset_definition_id,
    CANONICAL_XOR_ASSET_DEFINITION_ID,
  );
  assert.equal(
    request.provenance.signer,
    request.generated_service_provenance.signer,
  );
  assert.equal(
    request.provenance.signer,
    request.generated_apartment_provenance.signer,
  );
  assert.match(request.generated_service_provenance.signature, /^[0-9A-F]+$/);
  assert.match(request.generated_apartment_provenance.signature, /^[0-9A-F]+$/);
  assert.equal(Object.hasOwn(request, "private_key"), false);
});

test("buildSoraCloudHfDeployRequest rejects non-canonical lease asset aliases", () => {
  assert.throws(
    () =>
      buildSoraCloudHfDeployRequest({
        repoId: "sentence-transformers/all-MiniLM-L6-v2",
        revision: "main",
        modelName: "all-MiniLM-L6-v2",
        serviceName: "all_minilm_l6_v2",
        apartmentName: "all_minilm_agent",
        storageClass: "warm",
        leaseTermMs: "3600000",
        leaseAssetDefinitionId: "xor#universal",
        baseFeeNanos: "1",
        privateKeyHex: privateKeyHex(),
      }),
    /Asset Definition ID must be valid Base58/,
  );
});

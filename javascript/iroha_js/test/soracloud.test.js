import assert from "node:assert/strict";
import test from "node:test";

import {
  assembleSoracloudHfDeployRequest,
  buildSoracloudHfDeployDraft,
} from "../src/index.js";

const CANONICAL_XOR_ASSET_DEFINITION_ID = "61CtjvNd9T3THAR65GsMVHr82Bjc";

test("buildSoracloudHfDeployDraft returns unsigned payloads", () => {
  const draft = buildSoracloudHfDeployDraft({
    repoId: "sentence-transformers/all-MiniLM-L6-v2",
    revision: "main",
    modelName: "all-MiniLM-L6-v2",
    serviceName: "all_minilm_l6_v2",
    apartmentName: "all_minilm_agent",
    storageClass: "warm",
    leaseTermMs: "3600000",
    leaseAssetDefinitionId: CANONICAL_XOR_ASSET_DEFINITION_ID,
    baseFeeNanos: "1",
  });

  assert.equal(
    draft.payload.repo_id,
    "sentence-transformers/all-MiniLM-L6-v2",
  );
  assert.equal(draft.payload.service_name, "all_minilm_l6_v2");
  assert.equal(
    draft.payload.lease_asset_definition_id,
    CANONICAL_XOR_ASSET_DEFINITION_ID,
  );
  assert.equal(Object.hasOwn(draft, "privateKeyHex"), false);
  assert.equal(Object.hasOwn(draft, "private_key"), false);
  assert.equal(draft.provenancePayloads.deploy.label, "hf_deploy");
  assert.equal(
    draft.provenancePayloads.generatedApartment.payload.apartment_name,
    "all_minilm_agent",
  );
});

test("assembleSoracloudHfDeployRequest uses external provenances", () => {
  const draft = buildSoracloudHfDeployDraft({
    repoId: "sentence-transformers/all-MiniLM-L6-v2",
    modelName: "all-MiniLM-L6-v2",
    serviceName: "all_minilm_l6_v2",
    storageClass: "warm",
    leaseTermMs: "3600000",
    leaseAssetDefinitionId: CANONICAL_XOR_ASSET_DEFINITION_ID,
    baseFeeNanos: "1",
  });

  const request = assembleSoracloudHfDeployRequest(draft, {
    deploy: { signer: "signer", signature: "ABCD" },
    generatedService: { signer: "signer", signature: "CDEF" },
  });

  assert.equal(request.provenance.signature, "ABCD");
  assert.equal(request.generated_service_provenance.signature, "CDEF");
  assert.equal(Object.hasOwn(request, "generated_apartment_provenance"), false);
});

test("buildSoracloudHfDeployDraft rejects raw private keys", () => {
  assert.throws(
    () =>
      buildSoracloudHfDeployDraft({
        repoId: "sentence-transformers/all-MiniLM-L6-v2",
        modelName: "all-MiniLM-L6-v2",
        serviceName: "all_minilm_l6_v2",
        storageClass: "warm",
        leaseTermMs: "3600000",
        leaseAssetDefinitionId: CANONICAL_XOR_ASSET_DEFINITION_ID,
        baseFeeNanos: "1",
        privateKeyHex: "00",
      }),
    /privateKeyHex is not accepted/,
  );
});

test("buildSoracloudHfDeployDraft rejects non-canonical lease asset aliases", () => {
  assert.throws(
    () =>
      buildSoracloudHfDeployDraft({
        repoId: "sentence-transformers/all-MiniLM-L6-v2",
        revision: "main",
        modelName: "all-MiniLM-L6-v2",
        serviceName: "all_minilm_l6_v2",
        apartmentName: "all_minilm_agent",
        storageClass: "warm",
        leaseTermMs: "3600000",
        leaseAssetDefinitionId: "xor#universal",
        baseFeeNanos: "1",
      }),
    /Asset Definition ID must be valid Base58/,
  );
});

import assert from "node:assert/strict";
import test from "node:test";

import {
  assembleSoracloudAppInfraRequest,
  assembleSoracloudHfDeployRequest,
  buildSoracloudAppInfraDraft,
  buildSoracloudHfDeployDraft,
  buildSoracloudPrivateUploadedModelExecuteRequest,
  buildSoracloudPrivateUploadedModelReceiptQuery,
  deploySoracloudAppInfraInstruction,
  upgradeSoracloudAppInfraInstruction,
} from "../src/index.js";

const CANONICAL_XOR_ASSET_DEFINITION_ID = "61CtjvNd9T3THAR65GsMVHr82Bjc";
const HF_COMMIT_OID = "0123456789abcdef0123456789abcdef01234567";

function validHfDeployInput(overrides = {}) {
  return {
    repoId: "sentence-transformers/all-MiniLM-L6-v2",
    revision: HF_COMMIT_OID,
    modelName: "all-MiniLM-L6-v2",
    serviceName: "all_minilm_l6_v2",
    apartmentName: null,
    storageClass: "warm",
    leaseTermMs: "3600000",
    leaseAssetDefinitionId: CANONICAL_XOR_ASSET_DEFINITION_ID,
    baseFeeNanos: "1",
    ...overrides,
  };
}

function validPrivateArtifact(role, overrides = {}) {
  return {
    schemaVersion: 1,
    sorafsManifestDigest: `${role}-manifest-digest`,
    artifactHash: `${role}-artifact-hash`,
    ciphertextBytes: 64,
    artifactRole: role,
    ...overrides,
  };
}

function validPrivateExecuteInput(overrides = {}) {
  return {
    serviceName: "portal",
    serviceVersion: "1.0.0",
    weightVersion: "v1",
    modelId: "upload-1",
    modelName: null,
    bundleRoot: null,
    decryptionRequestId: "release-1",
    inputArtifact: validPrivateArtifact("input"),
    outputRecipient: {
      schemaVersion: 1,
      keyId: "client-output-key",
      keyVersion: 1,
      kem: "X25519HkdfSha256",
      aead: "Aes256Gcm",
      publicKeyBytes: "CQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
      publicKeyFingerprint: "client-output-key-fingerprint",
    },
    ...overrides,
  };
}

function validAppServiceInput(overrides = {}) {
  return {
    name: "portal",
    serviceVersion: "v1",
    serviceManifestHash: "hash-service",
    containerManifestHash: "hash-container",
    runtime: "Inrou",
    executionPlane: "HttpService",
    routes: [],
    leaseVolumes: [],
    shards: null,
    ...overrides,
  };
}

function validAppInfraInput(overrides = {}) {
  return {
    appName: "portal_app",
    appVersion: "v1",
    publicUrl: "https://portal.sora.example",
    staticSite: null,
    services: [validAppServiceInput()],
    ...overrides,
  };
}

test("buildSoracloudHfDeployDraft returns unsigned payloads", () => {
  const draft = buildSoracloudHfDeployDraft({
    repoId: "sentence-transformers/all-MiniLM-L6-v2",
    revision: HF_COMMIT_OID,
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

test("buildSoracloudHfDeployDraft requires an immutable canonical commit OID", () => {
  for (const revision of [
    undefined,
    "main",
    "0123456789abcdef",
    "0123456789ABCDEF0123456789ABCDEF01234567",
    ` ${HF_COMMIT_OID}`,
  ]) {
    assert.throws(
      () => buildSoracloudHfDeployDraft(validHfDeployInput({ revision })),
      /revision.*(?:non-empty|required|40-character lowercase hexadecimal commit OID)/,
    );
  }
});

test("buildSoracloudHfDeployDraft uses one exact input shape and explicit null apartment fields", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());
  assert.equal(draft.payload.apartment_name, null);
  assert.equal(draft.provenancePayloads.generatedApartment, null);

  const omittedApartment = validHfDeployInput();
  delete omittedApartment.apartmentName;
  assert.throws(
    () => buildSoracloudHfDeployDraft(omittedApartment),
    /input\.apartmentName is required/,
  );

  for (const alias of [
    "repo_id",
    "model_name",
    "service_name",
    "apartment_name",
    "storage_class",
    "lease_term_ms",
    "lease_asset_definition_id",
    "base_fee_nanos",
  ]) {
    assert.throws(
      () => buildSoracloudHfDeployDraft({ ...validHfDeployInput(), [alias]: "alias" }),
      new RegExp(`input\\.${alias} is not accepted`),
    );
  }

  const inherited = Object.create({ retiredField: true });
  Object.assign(inherited, validHfDeployInput());
  assert.throws(
    () => buildSoracloudHfDeployDraft(inherited),
    /input inherited properties are not accepted/,
  );

  const nonEnumerable = validHfDeployInput();
  Object.defineProperty(nonEnumerable, "apartmentName", {
    value: null,
    enumerable: false,
  });
  assert.throws(
    () => buildSoracloudHfDeployDraft(nonEnumerable),
    /input\.apartmentName must be enumerable/,
  );

  assert.throws(
    () =>
      buildSoracloudHfDeployDraft({
        ...validHfDeployInput(),
        [Symbol.for("retiredField")]: true,
      }),
    /input symbols are not accepted/,
  );
});

test("buildSoracloudAppInfraDraft expands sharded decentralized app services", () => {
  const draft = buildSoracloudAppInfraDraft({
    appName: "hayahi",
    appVersion: "2026.05.20",
    services: [
      {
        name: "hayahi_live",
        serviceVersion: "2026.05.20",
        serviceManifestHash: "hash-live-service",
        containerManifestHash: "hash-live-container",
        runtime: "Inrou",
        executionPlane: "HttpService",
        routes: [
          { path: "/api/v1", publicHost: "hayahi.sora", internalUrl: null },
        ],
        leaseVolumes: [],
        shards: null,
      },
      {
        name: "hayahi_data",
        serviceVersion: "2026.05.20",
        serviceManifestHash: "hash-data-service",
        containerManifestHash: "hash-data-container",
        runtime: "Inrou",
        executionPlane: "HttpService",
        routes: [],
        leaseVolumes: [
          {
            name: "owned_data",
            mountPath: "/var/lib/hayahi/data",
            maxTotalBytes: 536870912,
            temperature: "hot",
          },
        ],
        shards: null,
      },
      {
        name: "hayahi_crawler",
        serviceVersion: "2026.05.20",
        serviceManifestHash: "hash-crawler-service",
        containerManifestHash: "hash-crawler-container",
        runtime: "Inrou",
        executionPlane: "HttpService",
        routes: [],
        leaseVolumes: [],
        shards: {
          count: 8,
          shardIdEnv: "HAYAHI_CRAWLER_SHARD_ID",
          shardCountEnv: "HAYAHI_CRAWLER_SHARD_COUNT",
        },
      },
    ],
    publicUrl: "https://hayahi.sora.org",
    staticSite: {
      publicUrl: "https://hayahi.sora.org",
      contentCid: "bafyhayahi",
      manifestDigestHex: null,
      mountPath: "/",
      apiBasePath: "/api",
    },
  });

  assert.equal(draft.payload.app_name, "hayahi");
  assert.equal(draft.payload.public_url, "https://hayahi.sora.org");
  assert.equal(draft.payload.static_site.content_cid, "bafyhayahi");
  assert.equal(draft.payload.static_site.manifest_digest_hex, null);
  assert.equal(draft.payload.services.length, 10);
  assert.equal(draft.payload.services[2].service_name, "hayahi_crawler_00");
  assert.equal(draft.payload.services[9].service_name, "hayahi_crawler_07");
  assert.equal(draft.payload.services[9].shard, "HAYAHI_CRAWLER_SHARD_ID=7;HAYAHI_CRAWLER_SHARD_COUNT=8");
  assert.equal(draft.payload.services[1].lease_volumes[0], "owned_data");
  assert.deepEqual(draft.payload.services[1].routes, []);
  assert.equal(draft.payload.services[0].shard, null);
  assert.equal(draft.payload.services[0].routes[0].path_prefix, "/api/v1");
  assert.equal(draft.payload.services[0].routes[0].internal_url, null);
  assert.equal(draft.provenancePayloads.deploy.schema, "soracloud.app.infra.provenance.v1");
  assert.equal(draft.provenancePayloads.services.length, 10);
});

test("buildSoracloudAppInfraDraft rejects aliases, omissions, and inferred V1 values", () => {
  const canonical = buildSoracloudAppInfraDraft(validAppInfraInput());
  assert.equal(canonical.payload.static_site, null);
  assert.deepEqual(canonical.payload.services[0].routes, []);
  assert.deepEqual(canonical.payload.services[0].lease_volumes, []);
  assert.equal(canonical.payload.services[0].shard, null);

  for (const field of ["appVersion", "staticSite"]) {
    const input = validAppInfraInput();
    delete input[field];
    assert.throws(
      () => buildSoracloudAppInfraDraft(input),
      new RegExp(`input\\.${field} is required`),
    );
  }

  for (const field of ["runtime", "executionPlane", "routes", "leaseVolumes", "shards"]) {
    const service = validAppServiceInput();
    delete service[field];
    assert.throws(
      () => buildSoracloudAppInfraDraft(validAppInfraInput({ services: [service] })),
      new RegExp(`services\\[0\\]\\.${field} is required`),
    );
  }

  assert.throws(
    () =>
      buildSoracloudAppInfraDraft(
        validAppInfraInput({
          services: [validAppServiceInput({ executionPlane: "Ivm" })],
        }),
      ),
    /executionPlane must be HttpService or DeterministicService/,
  );

  for (const [scope, input] of [
    ["input.static_site", { ...validAppInfraInput(), static_site: null }],
    [
      "services[0].service_version",
      validAppInfraInput({
        services: [
          { ...validAppServiceInput(), service_version: "v1" },
        ],
      }),
    ],
    [
      "services[0].execution_plane",
      validAppInfraInput({
        services: [
          { ...validAppServiceInput(), execution_plane: "HttpService" },
        ],
      }),
    ],
    [
      "services[0].lease_volumes",
      validAppInfraInput({
        services: [
          { ...validAppServiceInput(), lease_volumes: [] },
        ],
      }),
    ],
  ]) {
    assert.throws(
      () => buildSoracloudAppInfraDraft(input),
      new RegExp(`${scope.replaceAll("[", "\\[").replaceAll("]", "\\]")} is not accepted`),
    );
  }

  const route = { path: "/api", publicHost: null, internalUrl: null };
  const routeDraft = buildSoracloudAppInfraDraft(
    validAppInfraInput({
      services: [validAppServiceInput({ routes: [route] })],
    }),
  );
  assert.equal(routeDraft.payload.services[0].routes[0].public_host, null);
  assert.equal(routeDraft.payload.services[0].routes[0].internal_url, null);
  for (const field of ["publicHost", "internalUrl"]) {
    const missing = { ...route };
    delete missing[field];
    assert.throws(
      () =>
        buildSoracloudAppInfraDraft(
          validAppInfraInput({
            services: [validAppServiceInput({ routes: [missing] })],
          }),
        ),
      new RegExp(`routes\\[0\\]\\.${field} is required`),
    );
  }
  assert.throws(
    () =>
      buildSoracloudAppInfraDraft(
        validAppInfraInput({
          services: [
            validAppServiceInput({
              routes: [{ ...route, public_host: "alias" }],
            }),
          ],
        }),
      ),
    /routes\[0\]\.public_host is not accepted/,
  );

  const staticSite = {
    publicUrl: "https://portal.sora.example",
    contentCid: null,
    manifestDigestHex: null,
    mountPath: "/",
    apiBasePath: null,
  };
  const staticDraft = buildSoracloudAppInfraDraft(
    validAppInfraInput({ staticSite }),
  );
  assert.equal(staticDraft.payload.static_site.content_cid, null);
  assert.equal(staticDraft.payload.static_site.manifest_digest_hex, null);
  assert.equal(staticDraft.payload.static_site.api_base_path, null);
  for (const field of ["contentCid", "manifestDigestHex", "mountPath", "apiBasePath"]) {
    const missing = { ...staticSite };
    delete missing[field];
    assert.throws(
      () => buildSoracloudAppInfraDraft(validAppInfraInput({ staticSite: missing })),
      new RegExp(`staticSite\\.${field} is required`),
    );
  }

  const volume = {
    name: "owned_data",
    mountPath: "/data",
    maxTotalBytes: 1024,
    temperature: "hot",
  };
  for (const field of ["mountPath", "maxTotalBytes", "temperature"]) {
    const missing = { ...volume };
    delete missing[field];
    assert.throws(
      () =>
        buildSoracloudAppInfraDraft(
          validAppInfraInput({
            services: [validAppServiceInput({ leaseVolumes: [missing] })],
          }),
        ),
      new RegExp(`leaseVolumes\\[0\\]\\.${field} is required`),
    );
  }

  const shards = { count: 2, shardIdEnv: "SHARD_ID", shardCountEnv: "SHARD_COUNT" };
  for (const field of ["shardIdEnv", "shardCountEnv"]) {
    const missing = { ...shards };
    delete missing[field];
    assert.throws(
      () =>
        buildSoracloudAppInfraDraft(
          validAppInfraInput({
            services: [validAppServiceInput({ shards: missing })],
          }),
        ),
      new RegExp(`shards\\.${field} is required`),
    );
  }

  const inherited = Object.create({ retiredField: true });
  Object.assign(inherited, validAppInfraInput());
  assert.throws(
    () => buildSoracloudAppInfraDraft(inherited),
    /input inherited properties are not accepted/,
  );
  const nonEnumerable = validAppInfraInput();
  Object.defineProperty(nonEnumerable, "staticSite", {
    value: null,
    enumerable: false,
  });
  assert.throws(
    () => buildSoracloudAppInfraDraft(nonEnumerable),
    /input\.staticSite must be enumerable/,
  );
});

test("buildSoracloudAppInfraDraft rejects nested signing secrets", () => {
  assert.throws(
    () =>
      buildSoracloudAppInfraDraft({
        appName: "bad",
        appVersion: "v1",
        publicUrl: "https://bad.example",
        staticSite: null,
        services: [
          {
            name: "bad_worker",
            serviceVersion: "v1",
            serviceManifestHash: "hash-service",
            containerManifestHash: "hash-container",
            runtime: "Inrou",
            executionPlane: "HttpService",
            routes: [
              {
                path: "/api",
                publicHost: null,
                internalUrl: null,
                privateKeyHex: "00",
              },
            ],
            leaseVolumes: [],
            shards: null,
          },
        ],
      }),
    /privateKeyHex is not accepted/,
  );
});

test("assembleSoracloudAppInfraRequest and instruction helpers use canonical manifest", () => {
  const draft = buildSoracloudAppInfraDraft({
    appName: "hayahi",
    appVersion: "2026.05.20",
    publicUrl: "https://hayahi.sora.org",
    staticSite: null,
    services: [
      {
        name: "hayahi_live",
        serviceVersion: "2026.05.20",
        serviceManifestHash: "hash-live-service",
        containerManifestHash: "hash-live-container",
        runtime: "Inrou",
        executionPlane: "HttpService",
        routes: [{ path: "/api", publicHost: null, internalUrl: null }],
        leaseVolumes: [],
        shards: null,
      },
    ],
  });
  const provenance = { signer: "signer", signature: "ABCD" };
  const request = assembleSoracloudAppInfraRequest(
    draft,
    { deploy: provenance },
    { deployServices: [], upgradeServices: [] },
  );

  assert.deepEqual(request.provenance, provenance);
  assert.equal(request.manifest.app_name, "hayahi");
  assert.deepEqual(request.deploy_services, []);
  assert.equal(
    deploySoracloudAppInfraInstruction(request.manifest, provenance).wire_id,
    "iroha_data_model::isi::soracloud::DeploySoracloudAppInfra",
  );
  assert.equal(
    upgradeSoracloudAppInfraInstruction(request.manifest, provenance).wire_id,
    "iroha_data_model::isi::soracloud::UpgradeSoracloudAppInfra",
  );
});

test("assembleSoracloudAppInfraRequest requires exact explicit service collections", () => {
  const draft = buildSoracloudAppInfraDraft(validAppInfraInput());
  const provenances = { deploy: { signer: "signer", signature: "ABCD" } };
  assert.throws(
    () => assembleSoracloudAppInfraRequest(draft, provenances),
    /options must be an object/,
  );
  assert.throws(
    () =>
      assembleSoracloudAppInfraRequest(draft, provenances, {
        deployServices: [],
      }),
    /options\.upgradeServices is required/,
  );
  assert.throws(
    () =>
      assembleSoracloudAppInfraRequest(draft, provenances, {
        deployServices: [],
        upgradeServices: [],
        deploy_services: [],
      }),
    /options\.deploy_services is not accepted/,
  );
});

test("assembleSoracloudHfDeployRequest uses external provenances", () => {
  const draft = buildSoracloudHfDeployDraft({
    repoId: "sentence-transformers/all-MiniLM-L6-v2",
    revision: HF_COMMIT_OID,
    modelName: "all-MiniLM-L6-v2",
    serviceName: "all_minilm_l6_v2",
    apartmentName: null,
    storageClass: "warm",
    leaseTermMs: "3600000",
    leaseAssetDefinitionId: CANONICAL_XOR_ASSET_DEFINITION_ID,
    baseFeeNanos: "1",
  });

  const provenances = {
    deploy: { signer: "signer", signature: "ABCD" },
    generatedService: { signer: "signer", signature: "CDEF" },
    generatedApartment: null,
  };
  const request = assembleSoracloudHfDeployRequest(draft, provenances);

  assert.equal(request.provenance.signature, "ABCD");
  assert.equal(request.generated_service_provenance.signature, "CDEF");
  assert.equal(request.generated_apartment_provenance, null);

  const { generatedApartment: _generatedApartment, ...omittedApartment } = provenances;
  assert.throws(
    () => assembleSoracloudHfDeployRequest(draft, omittedApartment),
    /generatedApartment provenance is required/,
  );
});

test("assembleSoracloudHfDeployRequest rejects unknown and non-enumerable provenance keys", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());
  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(draft, {
        deploy: { signer: "signer", signature: "ABCD", note: "alias" },
        generatedService: { signer: "signer", signature: "CDEF" },
      }),
    /deploy provenance\.note is not accepted/,
  );

  const deploy = { signer: "signer", signature: "ABCD" };
  Object.defineProperty(deploy, "signature", {
    value: "ABCD",
    enumerable: false,
  });
  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(draft, {
        deploy,
        generatedService: { signer: "signer", signature: "CDEF" },
      }),
    /deploy provenance\.signature must be enumerable/,
  );
});

test("buildSoracloudHfDeployDraft rejects raw private keys", () => {
  for (const field of ["privateKeyHex", "privateKey", "private_key", "private_key_hex"]) {
    assert.throws(
      () => buildSoracloudHfDeployDraft(validHfDeployInput({ [field]: "00" })),
      new RegExp(`${field} is not accepted`),
    );
  }
});

test("buildSoracloudHfDeployDraft rejects inherited signing secrets", () => {
  const input = Object.create({ privateKeyHex: "00" });
  Object.assign(input, validHfDeployInput());

  assert.throws(
    () => buildSoracloudHfDeployDraft(input),
    /privateKeyHex is not accepted/,
  );
});

test("assembleSoracloudHfDeployRequest rejects raw keys outside payloads", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());
  const provenances = {
    deploy: { signer: "signer", signature: "ABCD" },
    generatedService: { signer: "signer", signature: "CDEF" },
  };

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        { ...draft, privateKeyHex: "00" },
        provenances,
      ),
    /privateKeyHex is not accepted/,
  );

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        {
          ...draft,
          provenancePayloads: {
            ...draft.provenancePayloads,
            deploy: {
              ...draft.provenancePayloads.deploy,
              private_key_hex: "00",
            },
          },
        },
        provenances,
      ),
    /private_key_hex is not accepted/,
  );

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(draft, {
        ...provenances,
        privateKey: "00",
      }),
    /privateKey is not accepted/,
  );

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(draft, {
        ...provenances,
        deploy: { ...provenances.deploy, private_key: "00" },
      }),
    /private_key is not accepted/,
  );

  const inheritedSecretProvenance = Object.create({ privateKeyHex: "00" });
  Object.assign(inheritedSecretProvenance, provenances.deploy);
  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(draft, {
        ...provenances,
        deploy: inheritedSecretProvenance,
      }),
    /privateKeyHex is not accepted/,
  );
});

test("assembleSoracloudHfDeployRequest requires apartment provenance for apartment drafts", () => {
  const draft = buildSoracloudHfDeployDraft(
    validHfDeployInput({ apartmentName: "all_minilm_agent" }),
  );

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(draft, {
        deploy: { signer: "signer", signature: "ABCD" },
        generatedService: { signer: "signer", signature: "CDEF" },
      }),
    /generatedApartment provenance must include signer and signature/,
  );
});

test("assembleSoracloudHfDeployRequest rejects hand-built drafts without signing payloads", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        { payload: draft.payload },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /draft provenancePayloads\.deploy is required/,
  );
});

test("assembleSoracloudHfDeployRequest rejects inherited draft containers", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());
  const provenances = {
    deploy: { signer: "signer", signature: "ABCD" },
    generatedService: { signer: "signer", signature: "CDEF" },
  };
  const inheritedPayloadDraft = Object.create({
    payload: draft.payload,
    provenancePayloads: draft.provenancePayloads,
  });
  assert.throws(
    () => assembleSoracloudHfDeployRequest(inheritedPayloadDraft, provenances),
    /draft payload is required/,
  );

  const inheritedSigningPayloadsDraft = Object.create({
    provenancePayloads: draft.provenancePayloads,
  });
  inheritedSigningPayloadsDraft.payload = draft.payload;
  assert.throws(
    () => assembleSoracloudHfDeployRequest(inheritedSigningPayloadsDraft, provenances),
    /draft provenancePayloads\.deploy is required/,
  );
});

test("assembleSoracloudHfDeployRequest rejects inherited signing payload fields", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());
  const provenancePayloads = Object.create({
    deploy: draft.provenancePayloads.deploy,
  });
  provenancePayloads.generatedService = draft.provenancePayloads.generatedService;

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        { payload: draft.payload, provenancePayloads },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /draft provenancePayloads\.deploy is required/,
  );
});

test("assembleSoracloudHfDeployRequest rejects array-like signing containers", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());
  const provenancePayloads = [];
  provenancePayloads.deploy = draft.provenancePayloads.deploy;
  provenancePayloads.generatedService = draft.provenancePayloads.generatedService;

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        { payload: draft.payload, provenancePayloads },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /draft provenancePayloads\.deploy is required/,
  );
});

test("assembleSoracloudHfDeployRequest rejects inherited signing payload internals", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());
  const inheritedSchema = Object.create({
    schema: draft.provenancePayloads.deploy.schema,
  });
  inheritedSchema.label = draft.provenancePayloads.deploy.label;
  inheritedSchema.payload = draft.provenancePayloads.deploy.payload;

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        {
          ...draft,
          provenancePayloads: {
            ...draft.provenancePayloads,
            deploy: inheritedSchema,
          },
        },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /draft provenancePayloads\.deploy is required/,
  );

  const inheritedPayload = Object.create({ private_key: "00" });
  Object.assign(inheritedPayload, draft.provenancePayloads.deploy.payload);
  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        {
          ...draft,
          provenancePayloads: {
            ...draft.provenancePayloads,
            deploy: {
              ...draft.provenancePayloads.deploy,
              payload: inheritedPayload,
            },
          },
        },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /draft provenancePayloads\.deploy payload\.private_key must be an own property/,
  );

  const nonEnumerableOwnSecretPayload = {
    ...draft.provenancePayloads.deploy.payload,
  };
  Object.defineProperty(nonEnumerableOwnSecretPayload, "privateKeyHex", {
    value: "00",
    enumerable: false,
  });
  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        {
          ...draft,
          provenancePayloads: {
            ...draft.provenancePayloads,
            deploy: {
              ...draft.provenancePayloads.deploy,
              payload: nonEnumerableOwnSecretPayload,
            },
          },
        },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /privateKeyHex is not accepted/,
  );

  const nonEnumerableSecretPrototype = Object.create(null);
  Object.defineProperty(nonEnumerableSecretPrototype, "private_key_hex", {
    value: "00",
    enumerable: false,
  });
  const nonEnumerableInheritedSecretPayload = Object.create(
    nonEnumerableSecretPrototype,
  );
  Object.assign(
    nonEnumerableInheritedSecretPayload,
    draft.provenancePayloads.deploy.payload,
  );
  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        {
          ...draft,
          provenancePayloads: {
            ...draft.provenancePayloads,
            deploy: {
              ...draft.provenancePayloads.deploy,
              payload: nonEnumerableInheritedSecretPayload,
            },
          },
        },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /private_key_hex is not accepted/,
  );
});

test("assembleSoracloudHfDeployRequest rejects incomplete hand-built drafts with signing payloads", () => {
  const payload = {
    repo_id: "repo",
    revision: HF_COMMIT_OID,
    service_name: "service",
  };
  const draft = {
    payload,
    provenancePayloads: {
      deploy: {
        schema: "soracloud.hf.deploy.provenance.v1",
        label: "hf_deploy",
        payload,
      },
      generatedService: {
        schema: "soracloud.hf.deploy.provenance.v1",
        label: "generated_service",
        payload: {
          service_name: "service",
          repo_id: "repo",
          revision: HF_COMMIT_OID,
        },
      },
    },
  };

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(draft, {
        deploy: { signer: "signer", signature: "ABCD" },
        generatedService: { signer: "signer", signature: "CDEF" },
      }),
    /draft payload\.model_name must be a non-empty string/,
  );
});

test("assembleSoracloudHfDeployRequest rejects malformed draft payloads", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());

  const arrayDraft = [];
  arrayDraft.payload = draft.payload;
  arrayDraft.provenancePayloads = draft.provenancePayloads;
  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(arrayDraft, {
        deploy: { signer: "signer", signature: "ABCD" },
        generatedService: { signer: "signer", signature: "CDEF" },
      }),
    /draft payload is required/,
  );

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        { ...draft, payload: [] },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /draft payload is required/,
  );
});

test("assembleSoracloudHfDeployRequest rejects malformed canonical draft fields", () => {
  const malformed = [
    {
      payload: { storage_class: "archive" },
      message: /draft payload\.storage_class must be hot, warm, or cold/,
    },
    {
      payload: { lease_term_ms: "3600000" },
      message: /draft payload\.lease_term_ms must be a safe non-negative integer/,
    },
    {
      payload: { base_fee_nanos: "0001" },
      message: /draft payload\.base_fee_nanos must be a canonical non-negative integer string/,
    },
    {
      payload: { lease_asset_definition_id: "xor#universal" },
      message: /draft payload\.lease_asset_definition_id must be valid Base58/,
    },
  ];

  for (const { payload: overrides, message } of malformed) {
    const draft = buildSoracloudHfDeployDraft(validHfDeployInput());
    const payload = { ...draft.payload, ...overrides };
    assert.throws(
      () =>
        assembleSoracloudHfDeployRequest(
          { ...draft, payload },
          {
            deploy: { signer: "signer", signature: "ABCD" },
            generatedService: { signer: "signer", signature: "CDEF" },
          },
        ),
      message,
    );
  }
});

test("assembleSoracloudHfDeployRequest rejects draft payload mutation after signing", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());
  const tamperedDraft = {
    ...draft,
    payload: { ...draft.payload, service_name: "replayed_service" },
  };

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(tamperedDraft, {
        deploy: { signer: "signer", signature: "ABCD" },
        generatedService: { signer: "signer", signature: "CDEF" },
      }),
    /draft provenancePayloads\.deploy payload must match draft payload/,
  );

  const inPlaceDraft = buildSoracloudHfDeployDraft(validHfDeployInput());
  inPlaceDraft.payload.service_name = "replayed_service";
  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(inPlaceDraft, {
        deploy: { signer: "signer", signature: "ABCD" },
        generatedService: { signer: "signer", signature: "CDEF" },
      }),
    /draft provenancePayloads\.deploy payload must match draft payload/,
  );
});

test("assembleSoracloudHfDeployRequest rejects tampered generated service signing payloads", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());
  const tamperedDraft = {
    ...draft,
    provenancePayloads: {
      ...draft.provenancePayloads,
      generatedService: {
        ...draft.provenancePayloads.generatedService,
        payload: {
          ...draft.provenancePayloads.generatedService.payload,
          service_name: "replayed_service",
        },
      },
    },
  };

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(tamperedDraft, {
        deploy: { signer: "signer", signature: "ABCD" },
        generatedService: { signer: "signer", signature: "CDEF" },
      }),
    /draft provenancePayloads\.generatedService payload must match draft payload/,
  );
});

test("assembleSoracloudHfDeployRequest rejects unknown draft payload fields", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        { ...draft, payload: { ...draft.payload, private_key: "00" } },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /draft payload\.private_key is not accepted/,
  );

  const inheritedPayload = Object.create({ private_key: "00" });
  Object.assign(inheritedPayload, draft.payload);
  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        { ...draft, payload: inheritedPayload },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /draft payload\.private_key must be an own property/,
  );

  const symbolPayload = { ...draft.payload, [Symbol.for("private_key")]: "00" };
  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        { ...draft, payload: symbolPayload },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /draft payload symbols are not accepted/,
  );
});

test("buildSoracloudPrivateUploadedModelExecuteRequest normalizes encrypted execution requests", () => {
  const request = buildSoracloudPrivateUploadedModelExecuteRequest(
    validPrivateExecuteInput({ bundleRoot: "bundle-root" }),
  );

  assert.equal(request.service_name, "portal");
  assert.equal(request.service_version, "1.0.0");
  assert.equal(request.model_id, "upload-1");
  assert.equal(request.model_name, null);
  assert.equal(request.weight_version, "v1");
  assert.equal(request.bundle_root, "bundle-root");
  assert.equal(request.decryption_request_id, "release-1");
  assert.deepEqual(request.input_artifact, {
    schema_version: 1,
    sorafs_manifest_digest: "input-manifest-digest",
    artifact_hash: "input-artifact-hash",
    ciphertext_bytes: 64,
    artifact_role: "input",
  });
  assert.deepEqual(request.output_recipient, {
    schema_version: 1,
    key_id: "client-output-key",
    key_version: 1,
    kem: { kem: "X25519HkdfSha256", value: null },
    aead: { aead: "Aes256Gcm", value: null },
    public_key_bytes: "CQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
    public_key_fingerprint: "client-output-key-fingerprint",
  });
  assert.equal(Object.hasOwn(request, "private_key"), false);
});

test("buildSoracloudPrivateUploadedModelExecuteRequest rejects aliases, omissions, and exotic keys", () => {
  for (const field of [
    "serviceName",
    "serviceVersion",
    "weightVersion",
    "modelId",
    "modelName",
    "bundleRoot",
    "decryptionRequestId",
    "inputArtifact",
    "outputRecipient",
  ]) {
    const input = validPrivateExecuteInput();
    delete input[field];
    assert.throws(
      () => buildSoracloudPrivateUploadedModelExecuteRequest(input),
      new RegExp(`input\\.${field} is required`),
    );
  }

  for (const alias of [
    "service_name",
    "service_version",
    "weight_version",
    "model_id",
    "model_name",
    "bundle_root",
    "policy_id",
    "decryption_request_id",
    "plaintext_input_i32",
    "input_artifact",
    "output_artifact",
    "output_recipient",
    "emitted_sequence",
  ]) {
    assert.throws(
      () =>
        buildSoracloudPrivateUploadedModelExecuteRequest({
          ...validPrivateExecuteInput(),
          [alias]: "alias",
        }),
      new RegExp(`input\\.${alias} is not accepted`),
    );
  }

  for (const retiredField of [
    "policyId",
    "model",
    "plaintextInputI32",
    "outputArtifact",
    "emittedSequence",
  ]) {
    assert.throws(
      () =>
        buildSoracloudPrivateUploadedModelExecuteRequest({
          ...validPrivateExecuteInput(),
          [retiredField]: "retired",
        }),
      new RegExp(`input\\.${retiredField} is not accepted`),
    );
  }
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest({
        ...validPrivateExecuteInput(),
        inputArtifact: {
          ...validPrivateArtifact("input"),
          artifact_hash: "alias",
        },
      }),
    /inputArtifact\.artifact_hash is not accepted/,
  );

  const inherited = Object.create({ retiredField: true });
  Object.assign(inherited, validPrivateExecuteInput());
  assert.throws(
    () => buildSoracloudPrivateUploadedModelExecuteRequest(inherited),
    /input inherited properties are not accepted/,
  );
  const nonEnumerable = validPrivateExecuteInput();
  Object.defineProperty(nonEnumerable, "bundleRoot", {
    value: null,
    enumerable: false,
  });
  assert.throws(
    () => buildSoracloudPrivateUploadedModelExecuteRequest(nonEnumerable),
    /input\.bundleRoot must be enumerable/,
  );
});

test("buildSoracloudPrivateUploadedModelExecuteRequest accepts modelName selector", () => {
  const request = buildSoracloudPrivateUploadedModelExecuteRequest(
    validPrivateExecuteInput({
      modelId: null,
      modelName: "vision",
    }),
  );

  assert.equal(request.model_name, "vision");
  assert.equal(request.model_id, null);
});

test("buildSoracloudPrivateUploadedModelExecuteRequest rejects malformed encrypted requests", () => {
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({ modelName: "vision" }),
      ),
    /exactly one of modelId or modelName/,
  );
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          inputArtifact: validPrivateArtifact("output"),
        }),
      ),
    /inputArtifact\.artifactRole must be input/,
  );
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({ privateKeyHex: "00" }),
      ),
    /privateKeyHex is not accepted/,
  );
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          outputRecipient: {
            ...validPrivateExecuteInput().outputRecipient,
            publicKeyBytes: "not base64",
          },
        }),
      ),
    /publicKeyBytes must be canonical padded base64/,
  );
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          outputRecipient: {
            ...validPrivateExecuteInput().outputRecipient,
            kem: "RsaOaep",
          },
        }),
      ),
    /outputRecipient\.kem must be X25519HkdfSha256/,
  );
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          inputArtifact: validPrivateArtifact("input", { schemaVersion: 2 }),
        }),
      ),
    /inputArtifact\.schemaVersion must be 1/,
  );
});

test("buildSoracloudPrivateUploadedModelReceiptQuery normalizes filters", () => {
  const query = buildSoracloudPrivateUploadedModelReceiptQuery({
    receiptId: "receipt",
    serviceName: "portal",
    modelId: "upload-1",
    weightVersion: "v1",
    limit: "25",
    countMode: "exact",
  });

  assert.deepEqual(query, {
    receipt_id: "receipt",
    service_name: "portal",
    model_id: "upload-1",
    weight_version: "v1",
    limit: "25",
    count_mode: "exact",
  });
});

test("buildSoracloudPrivateUploadedModelReceiptQuery rejects unknown count mode", () => {
  assert.throws(
    () => buildSoracloudPrivateUploadedModelReceiptQuery({ countMode: "full" }),
    /countMode must be bounded or exact/,
  );
});

test("buildSoracloudPrivateUploadedModelReceiptQuery preserves genuine option omission only", () => {
  assert.deepEqual(buildSoracloudPrivateUploadedModelReceiptQuery({}), {});
  assert.throws(
    () => buildSoracloudPrivateUploadedModelReceiptQuery({ model_id: "alias" }),
    /input\.model_id is not accepted/,
  );
  const inherited = Object.create({ modelId: "upload-1" });
  assert.throws(
    () => buildSoracloudPrivateUploadedModelReceiptQuery(inherited),
    /input inherited properties are not accepted/,
  );
});

test("assembleSoracloudHfDeployRequest rejects inherited model and lease draft fields", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());
  const inheritedPayload = Object.create({
    model_name: draft.payload.model_name,
  });
  for (const [field, value] of Object.entries(draft.payload)) {
    if (field !== "model_name") {
      inheritedPayload[field] = value;
    }
  }

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        { ...draft, payload: inheritedPayload },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /draft payload\.model_name must be an own property/,
  );

  const requiredFieldPrototype = Object.create(null);
  Object.defineProperty(requiredFieldPrototype, "lease_term_ms", {
    value: draft.payload.lease_term_ms,
    enumerable: false,
  });
  const nonEnumerableInheritedPayload = Object.create(requiredFieldPrototype);
  for (const [field, value] of Object.entries(draft.payload)) {
    if (field !== "lease_term_ms") {
      nonEnumerableInheritedPayload[field] = value;
    }
  }
  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        { ...draft, payload: nonEnumerableInheritedPayload },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /draft payload\.lease_term_ms must be an own property/,
  );
});

test("assembleSoracloudHfDeployRequest rejects inherited revision and non-enumerable draft fields", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());
  const inheritedPayload = Object.create({
    revision: HF_COMMIT_OID,
  });
  for (const [field, value] of Object.entries(draft.payload)) {
    if (field !== "revision") {
      inheritedPayload[field] = value;
    }
  }

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        { ...draft, payload: inheritedPayload },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /draft payload\.revision must be an own property/,
  );

  const secretPrototype = Object.create(null);
  Object.defineProperty(secretPrototype, "private_key", {
    value: "00",
    enumerable: false,
  });
  const nonEnumerableSecretPayload = Object.create(secretPrototype);
  Object.assign(nonEnumerableSecretPayload, draft.payload);
  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        { ...draft, payload: nonEnumerableSecretPayload },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /draft payload\.private_key must be an own property/,
  );

  const nonEnumerableOwnPayload = { ...draft.payload };
  Object.defineProperty(nonEnumerableOwnPayload, "storage_class", {
    value: draft.payload.storage_class,
    enumerable: false,
  });
  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(
        { ...draft, payload: nonEnumerableOwnPayload },
        {
          deploy: { signer: "signer", signature: "ABCD" },
          generatedService: { signer: "signer", signature: "CDEF" },
        },
      ),
    /draft payload\.storage_class must be enumerable/,
  );
});

test("assembleSoracloudHfDeployRequest rejects inherited provenance fields", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());
  const inheritedProvenance = Object.create({
    signer: "signer",
    signature: "ABCD",
  });

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(draft, {
        deploy: inheritedProvenance,
        generatedService: { signer: "signer", signature: "CDEF" },
      }),
    /deploy provenance must include signer and signature/,
  );
});

test("assembleSoracloudHfDeployRequest rejects inherited provenance entries", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());
  const provenances = Object.create({
    deploy: { signer: "signer", signature: "ABCD" },
  });
  provenances.generatedService = { signer: "signer", signature: "CDEF" };

  assert.throws(
    () => assembleSoracloudHfDeployRequest(draft, provenances),
    /provenances inherited properties are not accepted/,
  );
});

test("assembleSoracloudHfDeployRequest rejects array-like provenances", () => {
  const draft = buildSoracloudHfDeployDraft(validHfDeployInput());
  const provenances = [];
  provenances.deploy = { signer: "signer", signature: "ABCD" };
  provenances.generatedService = { signer: "signer", signature: "CDEF" };

  assert.throws(
    () => assembleSoracloudHfDeployRequest(draft, provenances),
    /provenances must be an object/,
  );
});

test("assembleSoracloudHfDeployRequest rejects tampered apartment signing payloads", () => {
  const draft = buildSoracloudHfDeployDraft(
    validHfDeployInput({ apartmentName: "all_minilm_agent" }),
  );
  const tamperedDraft = {
    ...draft,
    provenancePayloads: {
      ...draft.provenancePayloads,
      generatedApartment: {
        ...draft.provenancePayloads.generatedApartment,
        label: "generated_service",
      },
    },
  };

  assert.throws(
    () =>
      assembleSoracloudHfDeployRequest(tamperedDraft, {
        deploy: { signer: "signer", signature: "ABCD" },
        generatedService: { signer: "signer", signature: "CDEF" },
        generatedApartment: { signer: "signer", signature: "DEFA" },
      }),
    /draft provenancePayloads\.generatedApartment is required/,
  );
});

test("buildSoracloudHfDeployDraft rejects adversarial numeric fields", () => {
  for (const overrides of [
    { leaseTermMs: "-1" },
    { leaseTermMs: "1.5" },
    { baseFeeNanos: "-1" },
    { baseFeeNanos: "1.5" },
  ]) {
    assert.throws(
      () => buildSoracloudHfDeployDraft(validHfDeployInput(overrides)),
      /must be a non-negative integer/,
    );
  }
});

test("buildSoracloudHfDeployDraft rejects unsafe lease integers", () => {
  assert.throws(
    () =>
      buildSoracloudHfDeployDraft(
        validHfDeployInput({ leaseTermMs: "9007199254740993" }),
      ),
    /leaseTermMs must fit in a safe JavaScript integer/,
  );
});

test("buildSoracloudHfDeployDraft rejects non-canonical lease asset aliases", () => {
  assert.throws(
    () =>
      buildSoracloudHfDeployDraft({
        repoId: "sentence-transformers/all-MiniLM-L6-v2",
        revision: HF_COMMIT_OID,
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

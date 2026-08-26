import assert from "node:assert/strict";
import test from "node:test";

import {
  assembleSoracloudAppInfraRequest,
  assembleSoracloudHfSharedLeaseJoinRequest,
  buildSoracloudAppInfraDraft,
  buildSoracloudHfSharedLeaseJoinDraft,
  deploySoracloudAppInfraInstruction,
  upgradeSoracloudAppInfraInstruction,
} from "../src/index.js";
import { canonicalHashLiteral } from "../src/instructionBuilderPrimitives.js";

const CANONICAL_XOR_ASSET_DEFINITION_ID = "61CtjvNd9T3THAR65GsMVHr82Bjc";
const HF_COMMIT_OID = "0123456789abcdef0123456789abcdef01234567";

function canonicalHash(nibble = "A") {
  const byte = Number.parseInt(`${nibble}${nibble}`, 16);
  return canonicalHashLiteral(Uint8Array.from({ length: 32 }, () => byte));
}

function validHfSharedLeaseJoinInput(overrides = {}) {
  return {
    repoId: "sentence-transformers/all-MiniLM-L6-v2",
    revision: HF_COMMIT_OID,
    serviceName: "all_minilm_l6_v2",
    apartmentName: null,
    storageClass: "warm",
    leaseTermMs: "3600000",
    leaseAssetDefinitionId: CANONICAL_XOR_ASSET_DEFINITION_ID,
    baseFee: "0.0000000001",
    ...overrides,
  };
}

function validAppServiceInput(overrides = {}) {
  return {
    name: "portal",
    serviceVersion: "v1",
    serviceManifestHash: canonicalHash("A"),
    containerManifestHash: canonicalHash("B"),
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

test("buildSoracloudHfSharedLeaseJoinDraft returns an unsigned storage-lease payload", () => {
  const draft = buildSoracloudHfSharedLeaseJoinDraft(
    validHfSharedLeaseJoinInput({ apartmentName: "all_minilm_agent" }),
  );

  assert.deepEqual(Object.keys(draft.payload), [
    "repo_id",
    "revision",
    "service_name",
    "apartment_name",
    "storage_class",
    "lease_term_ms",
    "lease_asset_definition_id",
    "base_fee",
  ]);
  assert.equal(draft.provenancePayloads.join.label, "hf_shared_lease_join");
  assert.equal(
    draft.provenancePayloads.join.schema,
    "soracloud.hf.shared_lease_join.provenance.v1",
  );
  assert.equal(Object.hasOwn(draft.payload, "model_name"), false);
  assert.equal(Object.hasOwn(draft.provenancePayloads, "generatedService"), false);
  assert.equal(
    buildSoracloudHfSharedLeaseJoinDraft(
      validHfSharedLeaseJoinInput({ baseFee: 1n }),
    ).payload.base_fee,
    "1",
  );
});

test("buildSoracloudHfSharedLeaseJoinDraft requires an immutable canonical commit OID", () => {
  for (const revision of [
    undefined,
    "main",
    "0123456789abcdef",
    "0123456789ABCDEF0123456789ABCDEF01234567",
    ` ${HF_COMMIT_OID}`,
  ]) {
    assert.throws(
      () =>
        buildSoracloudHfSharedLeaseJoinDraft(
          validHfSharedLeaseJoinInput({ revision }),
        ),
      /revision.*(?:non-empty|required|40-character lowercase hexadecimal commit OID)/,
    );
  }
});

test("buildSoracloudHfSharedLeaseJoinDraft rejects retired and ambiguous fields", () => {
  for (const overrides of [
    { repoId: " sentence-transformers/all-MiniLM-L6-v2" },
    { repoId: "all-MiniLM-L6-v2" },
    { repoId: "sentence-transformers//all-MiniLM-L6-v2" },
    { serviceName: " all_minilm_l6_v2" },
    { serviceName: "cafe\u0301" },
    { apartmentName: " ops_agent" },
    { apartmentName: "cafe\u0301" },
    { leaseAssetDefinitionId: ` ${CANONICAL_XOR_ASSET_DEFINITION_ID}` },
  ]) {
    assert.throws(
      () =>
        buildSoracloudHfSharedLeaseJoinDraft(
          validHfSharedLeaseJoinInput(overrides),
        ),
      /(?:canonical|fully-qualified|surrounding whitespace|without whitespace|Iroha Name)/,
    );
  }

  for (const alias of [
    "repo_id",
    "model_name",
    "modelName",
    "service_name",
    "apartment_name",
    "storage_class",
    "lease_term_ms",
    "lease_asset_definition_id",
    "baseFeeNanos",
    "base_fee_nanos",
  ]) {
    assert.throws(
      () =>
        buildSoracloudHfSharedLeaseJoinDraft({
          ...validHfSharedLeaseJoinInput(),
          [alias]: "alias",
        }),
      new RegExp(`input\\.${alias} is not accepted`),
    );
  }

  const inherited = Object.create({ retiredField: true });
  Object.assign(inherited, validHfSharedLeaseJoinInput());
  assert.throws(
    () => buildSoracloudHfSharedLeaseJoinDraft(inherited),
    /input inherited properties are not accepted/,
  );

  assert.throws(
    () =>
      buildSoracloudHfSharedLeaseJoinDraft({
        ...validHfSharedLeaseJoinInput(),
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
        serviceManifestHash: canonicalHash("1"),
        containerManifestHash: canonicalHash("2"),
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
        serviceManifestHash: canonicalHash("3"),
        containerManifestHash: canonicalHash("4"),
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
        serviceManifestHash: canonicalHash("5"),
        containerManifestHash: canonicalHash("6"),
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
  assert.deepEqual(draft.payload.services[0].execution_plane, {
    execution_plane: "HttpService",
    value: null,
  });
  assert.deepEqual(draft.payload.services[0].runtime, {
    runtime: "Inrou",
    value: null,
  });
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
            serviceManifestHash: canonicalHash("7"),
            containerManifestHash: canonicalHash("8"),
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
        serviceManifestHash: canonicalHash("9"),
        containerManifestHash: canonicalHash("A"),
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
    "iroha.instruction.v1::soracloud::DeploySoracloudAppInfra",
  );
  assert.equal(
    upgradeSoracloudAppInfraInstruction(request.manifest, provenance).wire_id,
    "iroha.instruction.v1::soracloud::UpgradeSoracloudAppInfra",
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
  assert.throws(
    () =>
      assembleSoracloudAppInfraRequest(
        { ...draft, retiredDraft: true },
        provenances,
        { deployServices: [], upgradeServices: [] },
      ),
    /draft\.retiredDraft is not accepted/,
  );
  assert.throws(
    () =>
      assembleSoracloudAppInfraRequest(
        {
          ...draft,
          provenancePayloads: {
            ...draft.provenancePayloads,
            legacy: null,
          },
        },
        provenances,
        { deployServices: [], upgradeServices: [] },
      ),
    /draft provenancePayloads\.legacy is not accepted/,
  );
});

test("assembleSoracloudHfSharedLeaseJoinRequest uses one external provenance", () => {
  const draft = buildSoracloudHfSharedLeaseJoinDraft(
    validHfSharedLeaseJoinInput(),
  );
  const request = assembleSoracloudHfSharedLeaseJoinRequest(draft, {
    join: { signer: "signer", signature: "ABCD" },
  });

  assert.deepEqual(Object.keys(request), ["payload", "provenance"]);
  assert.equal(request.provenance.signature, "ABCD");
  assert.equal(request.payload.repo_id, draft.payload.repo_id);
  assert.throws(
    () =>
      assembleSoracloudHfSharedLeaseJoinRequest(draft, {
        join: { signer: "signer", signature: "ABCD" },
        generatedService: { signer: "signer", signature: "CDEF" },
      }),
    /provenances\.generatedService is not accepted/,
  );
});

test("assembleSoracloudHfSharedLeaseJoinRequest closes the draft and provenance shapes", () => {
  const draft = buildSoracloudHfSharedLeaseJoinDraft(
    validHfSharedLeaseJoinInput(),
  );
  assert.throws(
    () =>
      assembleSoracloudHfSharedLeaseJoinRequest(
        { ...draft, retiredDraft: true },
        { join: { signer: "signer", signature: "ABCD" } },
      ),
    /draft\.retiredDraft is not accepted/,
  );
  assert.throws(
    () =>
      assembleSoracloudHfSharedLeaseJoinRequest(draft, {
        deploy: { signer: "signer", signature: "ABCD" },
      }),
    /provenances\.deploy is not accepted/,
  );
  assert.throws(
    () =>
      assembleSoracloudHfSharedLeaseJoinRequest(
        {
          ...draft,
          payload: { ...draft.payload, model_name: "retired" },
        },
        { join: { signer: "signer", signature: "ABCD" } },
      ),
    /draft payload\.model_name is not accepted/,
  );
});

test("Soracloud HF shared-lease join rejects raw signing secrets", () => {
  for (const field of ["privateKeyHex", "privateKey", "private_key", "private_key_hex"]) {
    assert.throws(
      () =>
        buildSoracloudHfSharedLeaseJoinDraft(
          validHfSharedLeaseJoinInput({ [field]: "00" }),
        ),
      /not accepted by the Soracloud JS API/,
    );
  }
});

test("Soracloud HF shared-lease join rejects adversarial economics", () => {
  for (const leaseTermMs of ["-1", "1.5", "0", "9007199254740993"]) {
    assert.throws(
      () =>
        buildSoracloudHfSharedLeaseJoinDraft(
          validHfSharedLeaseJoinInput({ leaseTermMs }),
        ),
      /leaseTermMs/,
    );
  }
  for (const baseFee of ["-1", "0.10", "0", 1]) {
    assert.throws(
      () =>
        buildSoracloudHfSharedLeaseJoinDraft(
          validHfSharedLeaseJoinInput({ baseFee }),
        ),
      /baseFee|canonical positive Quantity|KotodamaQuantity/,
    );
  }
  assert.throws(
    () =>
      buildSoracloudHfSharedLeaseJoinDraft(
        validHfSharedLeaseJoinInput({
          leaseAssetDefinitionId: "xor#universal",
        }),
      ),
    /canonical Base58 asset definition id/,
  );
});

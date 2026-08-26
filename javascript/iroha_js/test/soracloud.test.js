import assert from "node:assert/strict";
import test from "node:test";

import { computeHashLiteralCrc } from "../src/hashLiteralCrc.js";

import {
  AccountAddress,
  assembleSoracloudAppInfraRequest,
  assembleSoracloudHfDeployRequest,
  buildSoracloudAppInfraDraft,
  buildSoracloudHfDeployDraft,
  buildSoracloudPrivateUploadedModelExecuteRequest,
  buildSoracloudPrivateUploadedModelReceiptQuery,
  deploySoracloudAppInfraInstruction,
  normalizeSoracloudPrivateUploadedModelExecuteResponse,
  normalizeSoracloudPrivateUploadedModelExecutionReceipt,
  upgradeSoracloudAppInfraInstruction,
} from "../src/index.js";

const CANONICAL_XOR_ASSET_DEFINITION_ID = "61CtjvNd9T3THAR65GsMVHr82Bjc";
const HF_COMMIT_OID = "0123456789abcdef0123456789abcdef01234567";
const SORAFS_MANIFEST_DIGEST = Object.freeze(
  Array.from({ length: 32 }, (_, index) => index + 1),
);
const SORAFS_ROOT_CID = Object.freeze([
  1,
  0x71,
  0x1f,
  32,
  ...Array.from({ length: 32 }, (_, index) => index + 1),
]);
const PRIVATE_INPUT_ARTIFACT_HASH =
  "hash:EC5DA24E45DC3C5BEA0CBF476CFAB65090E350A5DEBB42655903315354B3C6AB#29EC";
const PRIVATE_BUNDLE_ROOT =
  "hash:C1333192F0B27FAA9F181BE020162911E4E9CCF70001E0802195427B76D04ABB#26C6";
const PRIVATE_RECEIPT_ID =
  "hash:F4043B977ED431CD60C92AF4B957085CA4D764544C54DD3017CCE8FCB56F7735#E4AA";
const PRIVATE_OUTPUT_KEY_FINGERPRINT =
  "hash:915A1442833BC2DF4DD5DA1C9616C015E1AA397D81BF30A01D8206051FCBC399#96C3";
const PRIVATE_UNMARKED_HASH_BODY = `${"00".repeat(31)}02`;
const PRIVATE_UNMARKED_HASH =
  `hash:${PRIVATE_UNMARKED_HASH_BODY}#${computeHashLiteralCrc(
    "hash",
    PRIVATE_UNMARKED_HASH_BODY,
  )}`;
const PRIVATE_ZERO_PREHASH_BODY = `${"00".repeat(31)}01`;
const PRIVATE_ZERO_PREHASH_HASH =
  `hash:${PRIVATE_ZERO_PREHASH_BODY}#${computeHashLiteralCrc(
    "hash",
    PRIVATE_ZERO_PREHASH_BODY,
  )}`;
const PRIVATE_MAX_CIPHERTEXT_BYTES = 72 * 1024 * 1024;
const U32_MAX = 0xffff_ffff;
const U64_MAX = 0xffff_ffff_ffff_ffffn;
const PRIVATE_NETWORK_ID =
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
const PRIVATE_OUTPUT_REPLICATION_ORDER_ID = Object.freeze([
  223, 84, 153, 93, 189, 208, 15, 57,
  18, 144, 6, 143, 35, 114, 49, 183,
  235, 169, 151, 26, 48, 191, 231, 173,
  2, 235, 241, 47, 189, 13, 37, 69,
]);
const PRIVATE_VALIDATOR_PUBLIC_KEY = Uint8Array.from(
  Buffer.from("5866666666666666666666666666666666666666666666666666666666666666", "hex"),
);
const PRIVATE_VALIDATOR_ACCOUNT_ID = AccountAddress.fromAccount({
  publicKey: PRIVATE_VALIDATOR_PUBLIC_KEY,
  algorithm: "ed25519",
}).toI105();
const PRIVATE_VALIDATOR_PEER_ID =
  `ed0120${Buffer.from(PRIVATE_VALIDATOR_PUBLIC_KEY).toString("hex").toUpperCase()}`;
const PRIVATE_WRAPPED_NONCE_BASE64 = "CwsLCwsLCwsLCwsL";
const PRIVATE_WRAPPED_KEY_CIPHERTEXT_BASE64 =
  "DAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM";
const PRIVATE_WRAPPED_KEY_CIPHERTEXT_HASH =
  "hash:3177350C03B71E0A4AB35017C6F1B4A041E23194FD7560A5B54147D2C1CED61B#CA93";

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
    sorafsManifestDigest: [...SORAFS_MANIFEST_DIGEST],
    sorafsRootCid: [...SORAFS_ROOT_CID],
    artifactHash: PRIVATE_INPUT_ARTIFACT_HASH,
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
    bundleRoot: PRIVATE_BUNDLE_ROOT,
    decryptionRequestId: "decrypt-upload-input",
    inputArtifact: validPrivateArtifact("input"),
    outputRecipient: {
      schemaVersion: 1,
      keyId: "client-output-key",
      keyVersion: 1,
      kem: "X25519HkdfSha256",
      aead: "Aes256Gcm",
      publicKeyBytes: "CQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
      publicKeyFingerprint: PRIVATE_OUTPUT_KEY_FINGERPRINT,
    },
    ...overrides,
  };
}

function privateHashLiteral(seed) {
  const marked = seed | 1;
  const body = marked.toString(16).toUpperCase().padStart(2, "0").repeat(32);
  return `hash:${body}#${computeHashLiteralCrc("hash", body)}`;
}

function validPrivateWireRecipient(overrides = {}) {
  return {
    schema_version: 1,
    key_id: "recipient-key",
    key_version: 1,
    kem: { kem: "X25519HkdfSha256", value: null },
    aead: { aead: "Aes256Gcm", value: null },
    public_key_bytes: "CQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
    public_key_fingerprint: PRIVATE_OUTPUT_KEY_FINGERPRINT,
    ...overrides,
  };
}

function validPrivateWireArtifact(role, digest, hash, ciphertextBytes) {
  return {
    schema_version: 1,
    sorafs_manifest_digest: Array(32).fill(digest),
    sorafs_root_cid: [...SORAFS_ROOT_CID],
    artifact_hash: hash,
    ciphertext_bytes: ciphertextBytes,
    artifact_role: role,
  };
}

function validPrivateExecutionReceipt(overrides = {}) {
  return {
    schema_version: 1,
    network_id: PRIVATE_NETWORK_ID,
    receipt_id: PRIVATE_RECEIPT_ID,
    service_name: "portal",
    service_version: "2026.1",
    model_id: "upload-1",
    weight_version: "v1",
    runtime_version: "soracloud.quantized-cpu.v1",
    model_manifest_digest: Array(32).fill(17),
    model_bundle_root: PRIVATE_BUNDLE_ROOT,
    policy_id: "policy-1",
    decryption_request_id: "decrypt-upload-1",
    attesting_validator: {
      lane_id: 0,
      validator_account_id: PRIVATE_VALIDATOR_ACCOUNT_ID,
      peer_id: PRIVATE_VALIDATOR_PEER_ID,
    },
    input_artifact: validPrivateWireArtifact(
      "input",
      34,
      PRIVATE_INPUT_ARTIFACT_HASH,
      64,
    ),
    output_artifact: validPrivateWireArtifact("output", 51, privateHashLiteral(5), 96),
    output_replication_order_id: [...PRIVATE_OUTPUT_REPLICATION_ORDER_ID],
    input_commitment: privateHashLiteral(7),
    output_commitment: privateHashLiteral(9),
    output_recipient: validPrivateWireRecipient(),
    request_commitment: privateHashLiteral(11),
    result_commitment: privateHashLiteral(13),
    authorization_claim_block_height: 0,
    authorization_claim_epoch: 0,
    emitted_sequence: 0,
    emitted_block_height: 0,
    emitted_epoch: 0,
    ...overrides,
  };
}

function validPrivateUploadedModelStatus(receipt, overrides = {}) {
  const chunkManifestRoot = privateHashLiteral(15);
  return {
    schema_version: 1,
    bundle: {
      schema_version: 1,
      service_name: receipt.service_name,
      model_id: receipt.model_id,
      weight_version: receipt.weight_version,
      family: "decoder-only",
      modalities: ["text"],
      plaintext_root: privateHashLiteral(17),
      runtime_format: {
        runtime_format: "DeterministicQuantizedCpuV1",
        value: null,
      },
      bundle_root: receipt.model_bundle_root,
      sorafs_manifest_digest: [...receipt.model_manifest_digest],
      chunk_count: 1,
      plaintext_bytes: 32,
      ciphertext_bytes: 48,
      chunk_manifest_root: chunkManifestRoot,
      upload_recipient: validPrivateWireRecipient({ key_id: "bundle-key" }),
      wrapped_bundle_key: {
        schema_version: 1,
        recipient_key_id: "bundle-key",
        recipient_key_version: 1,
        kem: { kem: "X25519HkdfSha256", value: null },
        aead: { aead: "Aes256Gcm", value: null },
        ephemeral_public_key: "CQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        nonce: PRIVATE_WRAPPED_NONCE_BASE64,
        wrapped_key_ciphertext: PRIVATE_WRAPPED_KEY_CIPHERTEXT_BASE64,
        ciphertext_hash: PRIVATE_WRAPPED_KEY_CIPHERTEXT_HASH,
        aad_digest: privateHashLiteral(19),
      },
      pricing_policy: { storage_price: "1" },
      decryption_policy_ref: receipt.policy_id,
    },
    artifact: {
      service_name: receipt.service_name,
      model_name: "portal_model",
      artifact_id: "artifact-1",
      training_job_id: "upload-1",
      weight_version: receipt.weight_version,
      weight_artifact_hash: PRIVATE_INPUT_ARTIFACT_HASH,
      dataset_ref: "dataset:upload-1",
      training_config_hash: privateHashLiteral(21),
      reproducibility_hash: privateHashLiteral(23),
      provenance_attestation_hash: privateHashLiteral(25),
      registered_sequence: 1,
      consumed_by_version: "v1",
      chunk_manifest_root: chunkManifestRoot,
    },
    ...overrides,
  };
}

function validPrivateExecuteResponse(overrides = {}) {
  const receipt = overrides.receipt ?? validPrivateExecutionReceipt();
  return {
    schema_version: 1,
    status: validPrivateUploadedModelStatus(receipt),
    submission_phase: "receipt_submitted",
    transaction_hash: privateHashLiteral(27),
    receipt,
    output_artifact: { ...receipt.output_artifact },
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
  const input = validPrivateExecuteInput();
  const request = buildSoracloudPrivateUploadedModelExecuteRequest(input);

  assert.deepEqual(request, {
    service_name: "portal",
    service_version: "1.0.0",
    weight_version: "v1",
    model_id: "upload-1",
    bundle_root: PRIVATE_BUNDLE_ROOT,
    decryption_request_id: "decrypt-upload-input",
    input_artifact: {
      schema_version: 1,
      sorafs_manifest_digest: [...SORAFS_MANIFEST_DIGEST],
      sorafs_root_cid: [...SORAFS_ROOT_CID],
      artifact_hash: PRIVATE_INPUT_ARTIFACT_HASH,
      ciphertext_bytes: 64,
      artifact_role: "input",
    },
    output_recipient: {
      schema_version: 1,
      key_id: "client-output-key",
      key_version: 1,
      kem: { kem: "X25519HkdfSha256", value: null },
      aead: { aead: "Aes256Gcm", value: null },
      public_key_bytes: "CQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
      public_key_fingerprint: PRIVATE_OUTPUT_KEY_FINGERPRINT,
    },
  });

  input.inputArtifact.sorafsManifestDigest[0] = 0xff;
  input.inputArtifact.sorafsRootCid[4] = 0xff;
  assert.equal(request.input_artifact.sorafs_manifest_digest[0], 1);
  assert.equal(request.input_artifact.sorafs_root_cid[4], 1);
  assert.equal(Object.hasOwn(request, "private_key"), false);
});

test("buildSoracloudPrivateUploadedModelExecuteRequest requires dense byte arrays", () => {
  class ExoticByteArray extends Array {}
  const subclassedDigest = new ExoticByteArray(...SORAFS_MANIFEST_DIGEST);
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          inputArtifact: validPrivateArtifact("input", {
            sorafsManifestDigest: subclassedDigest,
          }),
        }),
      ),
    /inputArtifact\.sorafsManifestDigest must be a plain array/,
  );

  const sparseDigest = [...SORAFS_MANIFEST_DIGEST];
  delete sparseDigest[7];
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          inputArtifact: validPrivateArtifact("input", {
            sorafsManifestDigest: sparseDigest,
          }),
        }),
      ),
    /inputArtifact\.sorafsManifestDigest\[7\] is required/,
  );

  const digestWithAlias = [...SORAFS_MANIFEST_DIGEST];
  digestWithAlias.byteLength = digestWithAlias.length;
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          inputArtifact: validPrivateArtifact("input", {
            sorafsManifestDigest: digestWithAlias,
          }),
        }),
      ),
    /inputArtifact\.sorafsManifestDigest\.byteLength is not accepted/,
  );

  let accessorRead = false;
  const accessorDigest = [...SORAFS_MANIFEST_DIGEST];
  Object.defineProperty(accessorDigest, "7", {
    configurable: true,
    enumerable: true,
    get() {
      accessorRead = true;
      return 8;
    },
  });
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          inputArtifact: validPrivateArtifact("input", {
            sorafsManifestDigest: accessorDigest,
          }),
        }),
      ),
    /inputArtifact\.sorafsManifestDigest\[7\] must be an enumerable data property/,
  );
  assert.equal(accessorRead, false, "byte-array accessors must be rejected before invocation");

  const hiddenDigest = [...SORAFS_MANIFEST_DIGEST];
  Object.defineProperty(hiddenDigest, "7", {
    configurable: true,
    enumerable: false,
    value: 8,
    writable: true,
  });
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          inputArtifact: validPrivateArtifact("input", {
            sorafsManifestDigest: hiddenDigest,
          }),
        }),
      ),
    /inputArtifact\.sorafsManifestDigest\[7\] must be an enumerable data property/,
  );
});

test("buildSoracloudPrivateUploadedModelExecuteRequest rejects aliases, omissions, and exotic keys", () => {
  for (const field of [
    "serviceName",
    "serviceVersion",
    "weightVersion",
    "modelId",
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
          [retiredField]: true,
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
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest({
        ...validPrivateExecuteInput(),
        inputArtifact: {
          ...validPrivateArtifact("input"),
          sorafs_root_cid: SORAFS_ROOT_CID,
        },
      }),
    /inputArtifact\.sorafs_root_cid is not accepted/,
  );
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest({
        ...validPrivateExecuteInput(),
        outputRecipient: {
          ...validPrivateExecuteInput().outputRecipient,
          public_key_fingerprint: PRIVATE_OUTPUT_KEY_FINGERPRINT,
        },
      }),
    /outputRecipient\.public_key_fingerprint is not accepted/,
  );

  const outputRecipientWithoutFingerprint = {
    ...validPrivateExecuteInput().outputRecipient,
  };
  delete outputRecipientWithoutFingerprint.publicKeyFingerprint;
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest({
        ...validPrivateExecuteInput(),
        outputRecipient: outputRecipientWithoutFingerprint,
      }),
    /outputRecipient\.publicKeyFingerprint is required/,
  );

  const inherited = Object.create({ retiredField: true });
  Object.assign(inherited, validPrivateExecuteInput());
  assert.throws(
    () => buildSoracloudPrivateUploadedModelExecuteRequest(inherited),
    /input inherited properties are not accepted/,
  );
  const nonEnumerable = validPrivateExecuteInput();
  Object.defineProperty(nonEnumerable, "bundleRoot", {
    value: PRIVATE_BUNDLE_ROOT,
    enumerable: false,
  });
  assert.throws(
    () => buildSoracloudPrivateUploadedModelExecuteRequest(nonEnumerable),
    /input\.bundleRoot must be enumerable/,
  );
});

test("buildSoracloudPrivateUploadedModelExecuteRequest rejects aliases, invalid release ids, artifacts, and secrets", () => {
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({ modelName: "vision" }),
      ),
    /input\.modelName is not accepted/,
  );
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({ modelId: null }),
      ),
    /modelId must be a non-empty string/,
  );
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({ bundleRoot: null }),
      ),
    /bundleRoot must be an exact uppercase checksummed marker-bit Iroha Hash literal/,
  );
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({ decryptionRequestId: null }),
      ),
    /decryptionRequestId must be a non-empty string/,
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
  const missingRootCid = validPrivateArtifact("input");
  delete missingRootCid.sorafsRootCid;
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({ inputArtifact: missingRootCid }),
      ),
    /inputArtifact\.sorafsRootCid is required/,
  );
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          inputArtifact: validPrivateArtifact("input", {
            sorafsRootCid: [1, 0x71, 0x1f, 32, 1],
          }),
        }),
      ),
    /inputArtifact\.sorafsRootCid must contain exactly 36 bytes/,
  );
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          inputArtifact: validPrivateArtifact("input", {
            sorafsManifestDigest: SORAFS_MANIFEST_DIGEST.slice(0, 31),
          }),
        }),
      ),
    /inputArtifact\.sorafsManifestDigest must contain exactly 32 bytes/,
  );
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          inputArtifact: validPrivateArtifact("input", {
            sorafsManifestDigest: [
              ...SORAFS_MANIFEST_DIGEST.slice(0, 31),
              256,
            ],
          }),
        }),
      ),
    /inputArtifact\.sorafsManifestDigest\[31\] must be an unsigned byte/,
  );
  for (const [sorafsRootCid, expectedError] of [
    [[2, 0x71, 0x1f, 32, ...SORAFS_ROOT_CID.slice(4)], /canonical CIDv1/],
    [[1, 0x71, 0x1f, 32, ...Array(32).fill(0)], /digest must not be all zero/],
    [[1, 0x71, 0x1f, 32, 1.5, ...SORAFS_ROOT_CID.slice(5)], /must be an unsigned byte/],
  ]) {
    assert.throws(
      () =>
        buildSoracloudPrivateUploadedModelExecuteRequest(
          validPrivateExecuteInput({
            inputArtifact: validPrivateArtifact("input", { sorafsRootCid }),
          }),
        ),
      expectedError,
    );
  }
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          outputRecipient: {
            ...validPrivateExecuteInput().outputRecipient,
            keyVersion: 0x1_0000_0000,
          },
        }),
      ),
    /outputRecipient\.keyVersion must fit in an unsigned 32-bit integer/,
  );
  for (const artifactHash of [
    "input-artifact-hash",
    PRIVATE_INPUT_ARTIFACT_HASH.toLowerCase(),
    ` ${PRIVATE_INPUT_ARTIFACT_HASH}`,
    PRIVATE_INPUT_ARTIFACT_HASH.replace(/.$/u, "0"),
    PRIVATE_UNMARKED_HASH,
    PRIVATE_ZERO_PREHASH_HASH,
  ]) {
    assert.throws(
      () =>
        buildSoracloudPrivateUploadedModelExecuteRequest(
          validPrivateExecuteInput({
            inputArtifact: validPrivateArtifact("input", { artifactHash }),
          }),
        ),
      /inputArtifact\.artifactHash (?:must be .*hash.* literal|has invalid checksum|must not be the zero prehash sentinel)/i,
    );
  }
  for (const bundleRoot of [
    "bundle-root",
    PRIVATE_BUNDLE_ROOT.toLowerCase(),
    ` ${PRIVATE_BUNDLE_ROOT}`,
    PRIVATE_BUNDLE_ROOT.replace(/.$/u, "0"),
    PRIVATE_UNMARKED_HASH,
    PRIVATE_ZERO_PREHASH_HASH,
  ]) {
    assert.throws(
      () =>
        buildSoracloudPrivateUploadedModelExecuteRequest(
          validPrivateExecuteInput({ bundleRoot }),
        ),
      /bundleRoot (?:must be .*hash.* literal|has invalid checksum|must not be the zero prehash sentinel)/i,
    );
  }
});

test("buildSoracloudPrivateUploadedModelExecuteRequest rejects string normalization", () => {
  for (const [input, expectedError] of [
    [validPrivateExecuteInput({ serviceName: " portal" }), /serviceName.*surrounding whitespace/],
    [validPrivateExecuteInput({ serviceName: "por tal" }), /serviceName.*canonical Iroha Name/],
    [validPrivateExecuteInput({ serviceName: "e\u0301" }), /serviceName.*NFC-normalized/],
    [validPrivateExecuteInput({ serviceName: "portal\uD800" }), /serviceName.*unpaired UTF-16/],
    [validPrivateExecuteInput({ serviceVersion: "1.0.0 " }), /serviceVersion.*surrounding whitespace/],
    [validPrivateExecuteInput({ serviceVersion: "1.0.0\uDFFF" }), /serviceVersion.*unpaired UTF-16/],
    [validPrivateExecuteInput({ weightVersion: "v 1" }), /weightVersion.*ASCII letters/],
    [validPrivateExecuteInput({ modelId: " upload-1" }), /modelId.*surrounding whitespace/],
    [
      validPrivateExecuteInput({ decryptionRequestId: "decrypt\nrequest" }),
      /decryptionRequestId.*control characters/,
    ],
    [
      validPrivateExecuteInput({ decryptionRequestId: "decrypt\uD800" }),
      /decryptionRequestId.*unpaired UTF-16/,
    ],
    [
      validPrivateExecuteInput({
        outputRecipient: {
          ...validPrivateExecuteInput().outputRecipient,
          keyId: " client-output-key",
        },
      }),
      /outputRecipient\.keyId.*surrounding whitespace/,
    ],
    [
      validPrivateExecuteInput({
        outputRecipient: {
          ...validPrivateExecuteInput().outputRecipient,
          keyId: "client-output-key\uDFFF",
        },
      }),
      /outputRecipient\.keyId.*unpaired UTF-16/,
    ],
    [
      validPrivateExecuteInput({
        outputRecipient: {
          ...validPrivateExecuteInput().outputRecipient,
          keyVersion: "01",
        },
      }),
      /outputRecipient\.keyVersion.*canonical positive decimal integer/,
    ],
    [
      validPrivateExecuteInput({
        outputRecipient: {
          ...validPrivateExecuteInput().outputRecipient,
          publicKeyBytes: " CQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        },
      }),
      /outputRecipient\.publicKeyBytes.*surrounding whitespace/,
    ],
  ]) {
    assert.throws(
      () => buildSoracloudPrivateUploadedModelExecuteRequest(input),
      expectedError,
    );
  }

  const scalarRequest = buildSoracloudPrivateUploadedModelExecuteRequest(
    validPrivateExecuteInput({
      serviceVersion: "release-\u{1F680}",
      decryptionRequestId: "decrypt-\u{1F680}",
      outputRecipient: {
        ...validPrivateExecuteInput().outputRecipient,
        keyId: "client-output-key-\u{1F680}",
      },
    }),
  );
  assert.equal(scalarRequest.service_version, "release-\u{1F680}");
  assert.equal(scalarRequest.decryption_request_id, "decrypt-\u{1F680}");
  assert.equal(scalarRequest.output_recipient.key_id, "client-output-key-\u{1F680}");
});

test("buildSoracloudPrivateUploadedModelExecuteRequest validates recipient key binding", () => {
  for (const publicKeyFingerprint of [
    PRIVATE_OUTPUT_KEY_FINGERPRINT.toLowerCase(),
    ` ${PRIVATE_OUTPUT_KEY_FINGERPRINT}`,
    PRIVATE_OUTPUT_KEY_FINGERPRINT.replace(/.$/u, "0"),
    PRIVATE_UNMARKED_HASH,
    PRIVATE_ZERO_PREHASH_HASH,
  ]) {
    assert.throws(
      () =>
        buildSoracloudPrivateUploadedModelExecuteRequest(
          validPrivateExecuteInput({
            outputRecipient: {
              ...validPrivateExecuteInput().outputRecipient,
              publicKeyFingerprint,
            },
          }),
        ),
      /outputRecipient\.publicKeyFingerprint (?:must be .*hash.* literal|has invalid checksum|must not be the zero prehash sentinel)/i,
    );
  }

  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          outputRecipient: {
            ...validPrivateExecuteInput().outputRecipient,
            publicKeyFingerprint: PRIVATE_INPUT_ARTIFACT_HASH,
          },
        }),
      ),
    /publicKeyFingerprint must equal the Iroha Blake2b-256 prehash/,
  );
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          outputRecipient: {
            ...validPrivateExecuteInput().outputRecipient,
            publicKeyBytes: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
          },
        }),
      ),
    /must not encode a low-order X25519 public key/,
  );
});

test("buildSoracloudPrivateUploadedModelExecuteRequest caps encrypted artifacts at 72 MiB", () => {
  const request = buildSoracloudPrivateUploadedModelExecuteRequest(
    validPrivateExecuteInput({
      inputArtifact: validPrivateArtifact("input", {
        ciphertextBytes: PRIVATE_MAX_CIPHERTEXT_BYTES,
      }),
    }),
  );
  assert.equal(
    request.input_artifact.ciphertext_bytes,
    PRIVATE_MAX_CIPHERTEXT_BYTES,
  );
  assert.throws(
    () =>
      buildSoracloudPrivateUploadedModelExecuteRequest(
        validPrivateExecuteInput({
          inputArtifact: validPrivateArtifact("input", {
            ciphertextBytes: "064",
          }),
        }),
      ),
    /inputArtifact\.ciphertextBytes.*canonical positive decimal integer/,
  );

  for (const ciphertextBytes of [
    0,
    PRIVATE_MAX_CIPHERTEXT_BYTES + 1,
    BigInt(PRIVATE_MAX_CIPHERTEXT_BYTES + 1),
    String(PRIVATE_MAX_CIPHERTEXT_BYTES + 1),
  ]) {
    assert.throws(
      () =>
        buildSoracloudPrivateUploadedModelExecuteRequest(
          validPrivateExecuteInput({
            inputArtifact: validPrivateArtifact("input", { ciphertextBytes }),
          }),
        ),
      /inputArtifact\.ciphertextBytes must (?:be greater than zero|be between 1 and 75497472)/,
    );
  }
});

test("buildSoracloudPrivateUploadedModelReceiptQuery normalizes filters", () => {
  const cursor = "A".repeat(114);
  const query = buildSoracloudPrivateUploadedModelReceiptQuery({
    receiptId: PRIVATE_RECEIPT_ID,
    serviceName: "portal",
    modelId: "upload-1",
    weightVersion: "v1",
    cursor,
    limit: "25",
    countMode: "exact",
  });

  assert.deepEqual(query, {
    receipt_id: PRIVATE_RECEIPT_ID,
    service_name: "portal",
    model_id: "upload-1",
    weight_version: "v1",
    cursor,
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

test("buildSoracloudPrivateUploadedModelReceiptQuery rejects normalized filters", () => {
  for (const [query, expectedError] of [
    [{ serviceName: " portal" }, /serviceName.*surrounding whitespace/],
    [{ serviceName: "e\u0301" }, /serviceName.*NFC-normalized/],
    [{ modelId: "upload 1" }, /modelId.*ASCII letters/],
    [{ weightVersion: "v1 " }, /weightVersion.*surrounding whitespace/],
    [{ countMode: " exact" }, /countMode.*surrounding whitespace/],
    [{ cursor: "short" }, /cursor must be an exact canonical V1 receipt cursor/],
    [{ cursor: `${"A".repeat(113)}=` }, /cursor must be an exact canonical V1 receipt cursor/],
    [{ limit: "025" }, /limit.*canonical positive decimal integer/],
  ]) {
    assert.throws(
      () => buildSoracloudPrivateUploadedModelReceiptQuery(query),
      expectedError,
    );
  }
});

test("buildSoracloudPrivateUploadedModelReceiptQuery enforces canonical hashes and the route limit", () => {
  assert.equal(
    buildSoracloudPrivateUploadedModelReceiptQuery({ limit: 500 }).limit,
    "500",
  );
  assert.throws(
    () => buildSoracloudPrivateUploadedModelReceiptQuery({ limit: 0 }),
    /limit must be greater than zero/,
  );
  assert.throws(
    () => buildSoracloudPrivateUploadedModelReceiptQuery({ limit: 501 }),
    /limit must be between 1 and 500/,
  );
  for (const receiptId of [
    "receipt",
    PRIVATE_RECEIPT_ID.toLowerCase(),
    ` ${PRIVATE_RECEIPT_ID}`,
    PRIVATE_RECEIPT_ID.replace(/.$/u, "0"),
    PRIVATE_UNMARKED_HASH,
    PRIVATE_ZERO_PREHASH_HASH,
  ]) {
    assert.throws(
      () => buildSoracloudPrivateUploadedModelReceiptQuery({ receiptId }),
      /receiptId (?:must be .*hash.* literal|has invalid checksum|must not be the zero prehash sentinel)/i,
    );
  }
});

test("buildSoracloudPrivateUploadedModelReceiptQuery preserves genuine option omission only", () => {
  assert.deepEqual(buildSoracloudPrivateUploadedModelReceiptQuery({}), {});
  for (const field of [
    "receiptId",
    "serviceName",
    "modelId",
    "weightVersion",
    "cursor",
    "limit",
    "countMode",
  ]) {
    assert.throws(
      () => buildSoracloudPrivateUploadedModelReceiptQuery({ [field]: null }),
      new RegExp(field),
    );
  }
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

test("normalizeSoracloudPrivateUploadedModelExecuteResponse accepts every exact phase shape", () => {
  for (const [submissionPhase, transactionHash] of [
    ["awaiting_output_durability", null],
    ["prepare_submitted", privateHashLiteral(27)],
    ["receipt_submitted", privateHashLiteral(29)],
  ]) {
    const normalized = normalizeSoracloudPrivateUploadedModelExecuteResponse(
      validPrivateExecuteResponse({
        submission_phase: submissionPhase,
        transaction_hash: transactionHash,
      }),
    );
    assert.equal(normalized.submission_phase, submissionPhase);
    assert.equal(normalized.transaction_hash, transactionHash);
    assert.equal(normalized.receipt.authorization_claim_block_height, 0);
    assert.equal(normalized.receipt.authorization_claim_epoch, 0);
    assert.equal(normalized.receipt.emitted_sequence, 0);
    assert.equal(normalized.receipt.emitted_block_height, 0);
    assert.equal(normalized.receipt.emitted_epoch, 0);
    assert.ok(Object.isFrozen(normalized));
    assert.ok(Object.isFrozen(normalized.status));
    assert.ok(Object.isFrozen(normalized.status.bundle));
    assert.ok(Object.isFrozen(normalized.receipt));
    assert.ok(Object.isFrozen(normalized.output_artifact));
  }

  const committedReceipt = validPrivateExecutionReceipt({
    authorization_claim_block_height: U64_MAX,
    authorization_claim_epoch: U64_MAX,
    emitted_sequence: U64_MAX,
    emitted_block_height: U64_MAX,
    emitted_epoch: U64_MAX,
  });
  const committed = normalizeSoracloudPrivateUploadedModelExecuteResponse(
    validPrivateExecuteResponse({
      submission_phase: "committed",
      transaction_hash: null,
      receipt: committedReceipt,
    }),
  );
  assert.equal(committed.submission_phase, "committed");
  assert.equal(committed.transaction_hash, null);
  assert.equal(committed.receipt.authorization_claim_block_height, U64_MAX);
  assert.equal(committed.receipt.authorization_claim_epoch, U64_MAX);
  assert.equal(committed.receipt.emitted_sequence, U64_MAX);
  assert.equal(committed.receipt.emitted_block_height, U64_MAX);
  assert.equal(committed.receipt.emitted_epoch, U64_MAX);
});

test("normalizeSoracloudPrivateUploadedModelExecutionReceipt enforces the tagged automatic order ID", () => {
  const normalized = normalizeSoracloudPrivateUploadedModelExecutionReceipt(
    validPrivateExecutionReceipt(),
  );
  assert.deepEqual(
    normalized.output_replication_order_id,
    PRIVATE_OUTPUT_REPLICATION_ORDER_ID,
  );
  assert.ok((normalized.output_replication_order_id[0] & 0x80) !== 0);

  const mismatched = validPrivateExecutionReceipt();
  mismatched.output_replication_order_id[31] ^= 1;
  assert.throws(
    () => normalizeSoracloudPrivateUploadedModelExecutionReceipt(mismatched),
    /tagged automatic replication-order ID/,
  );
});

test("normalizeSoracloudPrivateUploadedModelExecuteResponse rejects phase and ledger-state mismatches", () => {
  for (const response of [
    validPrivateExecuteResponse({
      submission_phase: "awaiting_output_durability",
      transaction_hash: privateHashLiteral(27),
    }),
    validPrivateExecuteResponse({
      submission_phase: "prepare_submitted",
      transaction_hash: null,
    }),
    validPrivateExecuteResponse({
      submission_phase: "receipt_submitted",
      receipt: validPrivateExecutionReceipt({
        authorization_claim_block_height: 1,
        authorization_claim_epoch: 1,
        emitted_sequence: 1,
        emitted_block_height: 1,
        emitted_epoch: 1,
      }),
    }),
    validPrivateExecuteResponse({
      submission_phase: "committed",
      transaction_hash: null,
    }),
  ]) {
    assert.throws(
      () => normalizeSoracloudPrivateUploadedModelExecuteResponse(response),
      /transaction_hash|ledger coordinates/,
    );
  }

  assert.throws(
    () =>
      normalizeSoracloudPrivateUploadedModelExecuteResponse(
        validPrivateExecuteResponse({ submission_phase: "submitted" }),
      ),
    /closed first-release phase/,
  );
});

test("normalizeSoracloudPrivateUploadedModelExecuteResponse rejects aliases and non-data fields", () => {
  const alias = validPrivateExecuteResponse();
  delete alias.submission_phase;
  alias.submission_status = "receipt_submitted";
  assert.throws(
    () => normalizeSoracloudPrivateUploadedModelExecuteResponse(alias),
    /submission_status is not accepted/,
  );

  const accessor = validPrivateExecuteResponse();
  Object.defineProperty(accessor, "submission_phase", {
    enumerable: true,
    get: () => "receipt_submitted",
  });
  assert.throws(
    () => normalizeSoracloudPrivateUploadedModelExecuteResponse(accessor),
    /submission_phase must be an enumerable data property/,
  );

  const inherited = Object.create(validPrivateExecuteResponse());
  assert.throws(
    () => normalizeSoracloudPrivateUploadedModelExecuteResponse(inherited),
    /inherited properties are not accepted/,
  );
});

test("normalizeSoracloudPrivateUploadedModelExecuteResponse enforces exact nested status and receipt bindings", () => {
  const extraBundleField = validPrivateExecuteResponse();
  extraBundleField.status.bundle.legacy_model_name = "upload-1";
  assert.throws(
    () => normalizeSoracloudPrivateUploadedModelExecuteResponse(extraBundleField),
    /legacy_model_name is not accepted/,
  );

  const mismatchedModel = validPrivateExecuteResponse();
  mismatchedModel.status.bundle.model_id = "upload-2";
  assert.throws(
    () => normalizeSoracloudPrivateUploadedModelExecuteResponse(mismatchedModel),
    /bundle\.model_id must match receipt/,
  );

  const mismatchedManifest = validPrivateExecuteResponse();
  mismatchedManifest.status.bundle.sorafs_manifest_digest[0] ^= 1;
  assert.throws(
    () => normalizeSoracloudPrivateUploadedModelExecuteResponse(mismatchedManifest),
    /bundle\.sorafs_manifest_digest must match receipt/,
  );

  const mismatchedArtifact = validPrivateExecuteResponse();
  mismatchedArtifact.status.artifact.chunk_manifest_root = privateHashLiteral(31);
  assert.throws(
    () => normalizeSoracloudPrivateUploadedModelExecuteResponse(mismatchedArtifact),
    /artifact\.chunk_manifest_root must match bundle/,
  );

  const mismatchedOutput = validPrivateExecuteResponse();
  mismatchedOutput.output_artifact.artifact_hash = privateHashLiteral(33);
  assert.throws(
    () => normalizeSoracloudPrivateUploadedModelExecuteResponse(mismatchedOutput),
    /output_artifact must match receipt\.output_artifact/,
  );
});

test("private uploaded-model response normalizers require lossless u64 inputs", () => {
  const coordinateFields = [
    "authorization_claim_block_height",
    "authorization_claim_epoch",
    "emitted_sequence",
    "emitted_block_height",
    "emitted_epoch",
  ];
  for (const coordinate of coordinateFields) {
    for (const invalid of [Number(U64_MAX), U64_MAX.toString()]) {
      assert.throws(
        () => normalizeSoracloudPrivateUploadedModelExecutionReceipt(
          validPrivateExecutionReceipt({ [coordinate]: invalid }),
        ),
        new RegExp(`${coordinate} must be a lossless unsigned integer`, "u"),
      );
    }
  }

  for (const coordinate of coordinateFields) {
    assert.throws(
      () => normalizeSoracloudPrivateUploadedModelExecutionReceipt(
        validPrivateExecutionReceipt({ [coordinate]: 1 }),
      ),
      /ledger coordinates must all be zero or all be positive/,
    );
  }
});

test("private uploaded-model receipts require every ledger coordinate in canonical order", () => {
  const coordinateFields = [
    "authorization_claim_block_height",
    "authorization_claim_epoch",
    "emitted_sequence",
    "emitted_block_height",
    "emitted_epoch",
  ];
  for (const coordinate of coordinateFields) {
    const receipt = validPrivateExecutionReceipt();
    delete receipt[coordinate];
    assert.throws(
      () => normalizeSoracloudPrivateUploadedModelExecutionReceipt(receipt),
      new RegExp(`${coordinate} is required`, "u"),
    );
  }

  for (const receipt of [
    validPrivateExecutionReceipt({
      authorization_claim_block_height: 2,
      authorization_claim_epoch: 1,
      emitted_sequence: 1,
      emitted_block_height: 1,
      emitted_epoch: 1,
    }),
    validPrivateExecutionReceipt({
      authorization_claim_block_height: 1,
      authorization_claim_epoch: 2,
      emitted_sequence: 1,
      emitted_block_height: 1,
      emitted_epoch: 1,
    }),
  ]) {
    assert.throws(
      () => normalizeSoracloudPrivateUploadedModelExecutionReceipt(receipt),
      /emission coordinates must not precede authorization claim coordinates/,
    );
  }
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

import { test } from "node:test";
import assert from "node:assert/strict";
import crypto from "node:crypto";
import { readFileSync } from "node:fs";
import fs from "node:fs/promises";
import path from "node:path";
import {
  ToriiClient,
  ToriiDataModelCompatibilityError,
  ToriiHttpError,
  extractPipelineStatusKind,
  decodePdpCommitmentHeader,
  TransactionStatusError,
  TransactionTimeoutError,
  IsoMessageTimeoutError,
  buildRbcSampleRequest,
} from "../src/toriiClient.js";
import {
  resolveToriiClientConfig,
  extractToriiFeatureConfig,
  extractConfidentialGasConfig,
} from "../src/config.js";
import {
  normalizeAccountId,
  ValidationError,
  ValidationErrorCode,
} from "../src/index.js";
import {
  AccountAddress,
  AccountAddressError,
  AccountAddressErrorCode,
} from "../src/address.js";
import { sorafsGatewayFetch } from "../src/sorafs.js";
import { getNativeBinding } from "../src/native.js";
import { makeNativeTest, nativeSkipMessage } from "./helpers/native.js";

const BASE_URL = "https://localhost:8080";
const SAMPLE_ACCOUNT_SIGNATORY =
  "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";
const SAMPLE_ACCOUNT_DOMAIN = "wonderland";
const SORA_I105_DISCRIMINANT = 0x2f1;
const SAMPLE_CONNECT_SID_BASE64 = bufferToBase64Url(Buffer.alloc(32, 0xcd));
const toriiFixtures = JSON.parse(
  readFileSync(new URL("./fixtures/torii_responses.json", import.meta.url), "utf8"),
);
const validationFixtures = JSON.parse(
  readFileSync(new URL("./fixtures/validation_errors.json", import.meta.url), "utf8"),
);
const txStatusErrorMessageContract = JSON.parse(
  readFileSync(
    new URL("../../../fixtures/sdk/tx_status_error_message_contract.json", import.meta.url),
    "utf8",
  ),
);
const nativeTest = makeNativeTest(test);

function cloneFixture(value) {
  return JSON.parse(JSON.stringify(value));
}

function sampleAccountForms() {
  const publicKeyHex = SAMPLE_ACCOUNT_SIGNATORY.slice(6);
  const publicKey = Buffer.from(publicKeyHex, "hex");
  const address = AccountAddress.fromAccount({
    domain: SAMPLE_ACCOUNT_DOMAIN,
    publicKey,
  });
  const i105Literal = address.toI105(SORA_I105_DISCRIMINANT);
  const nonCanonicalI105 = address.toI105(0x02f2);
  const canonical = normalizeAccountId(
    i105Literal,
    "toriiClient.sampleAccountForms",
  );
  const canonicalBytes = Buffer.from(address.canonicalHex().slice(2), "hex");
  const digestStart = 2;
  const truncated = Buffer.concat([
    canonicalBytes.subarray(0, digestStart + 8),
    canonicalBytes.subarray(digestStart + 12),
  ]);
  const local8 = `0x${truncated.toString("hex")}`;
  return Object.freeze({
    canonical,
    i105: i105Literal,
    i105Default: address.toI105Default(),
    nonCanonicalI105,
    local8,
  });
}

const SAMPLE_ACCOUNT_FORMS = sampleAccountForms();
const SAMPLE_ACCOUNT_ID = SAMPLE_ACCOUNT_FORMS.canonical;

function fixtureAccountAddress(label, domain = "fixture-domain") {
  let attempt = 0;
  while (attempt < 1024) {
    const hash = crypto
      .createHash("sha256")
      .update(`fixture:${label}@${domain}:${attempt}`)
      .digest();
    try {
      return AccountAddress.fromAccount({
        domain: SAMPLE_ACCOUNT_DOMAIN,
        publicKey: hash,
      });
    } catch (error) {
      if (
        !(error instanceof AccountAddressError) ||
        error.code !== AccountAddressErrorCode.INVALID_PUBLIC_KEY
      ) {
        throw error;
      }
      attempt += 1;
    }
  }
  throw new Error(`unable to derive canonical fixture key for ${label}@${domain}`);
}

function fixtureAccountId(label, domain = "fixture-domain") {
  return fixtureAccountAddress(label, domain).toI105(SORA_I105_DISCRIMINANT);
}

function fixtureAccountForms(label, domain = "fixture-domain") {
  const address = fixtureAccountAddress(label, domain);
  return {
    i105: address.toI105(SORA_I105_DISCRIMINANT),
    i105Default: address.toI105Default(),
  };
}

const FIXTURE_ALICE_ID = fixtureAccountId("alice");
const FIXTURE_BOB_ID = fixtureAccountId("bob");
const FIXTURE_CAROL_ID = fixtureAccountId("carol");
const FIXTURE_ALICE_TEST_ID = fixtureAccountId("alice", "test");
const FIXTURE_VALIDATOR_TEST_ID = fixtureAccountId("validator", "test");
const FIXTURE_BOB_NARNIA_ID = fixtureAccountId("bob", "narnia");
const FIXTURE_VAULT_ID = fixtureAccountId("vault");
const FIXTURE_MERCHANT_ID = fixtureAccountId("merchant");
const FIXTURE_ISSUER_ID = fixtureAccountId("issuer");
const FIXTURE_AUTHORITY_ID = fixtureAccountId("authority");
const FIXTURE_COUNCIL_TEST_ID = fixtureAccountId("council", "test");
const FIXTURE_ASSET_ID_A = "norito:01020304deadbeef";
const FIXTURE_ASSET_ID_B = "norito:01020304cafebabe";
const FIXTURE_ASSET_ID_C = "norito:01020304feedface";
const FIXTURE_ASSET_ID_D = "norito:01020304aabbccdd";

function expectValidationErrorFixture(error, key) {
  assert(error instanceof ValidationError);
  const fixture = validationFixtures[key];
  assert.ok(fixture, `missing validation error fixture: ${key}`);
  assert.equal(error.code, fixture.code);
  assert.equal(error.path, fixture.path);
  assert.equal(error.message, fixture.message);
  return true;
}

const SAMPLE_SNS_GOV_CASE_RESPONSE = Object.freeze({
  case_id: "SNS-2026-00001",
  selector: { suffix_id: 42, label: "alice", global_form: "alice.sora" },
  dispute_type: "ownership",
  priority: "urgent",
  reported_at: "2026-04-01T00:00:00Z",
  acknowledged_at: "2026-04-01T01:00:00Z",
  triage_started_at: "2026-04-01T01:30:00Z",
  hearing_scheduled_at: null,
  resolution_issued_at: null,
  status: "open",
  reporter: { role: "registrar", contact: "ops@example.com", reference_ticket: "SUP-1" },
  respondents: [
    {
      role: "registrant",
      account_id: FIXTURE_ALICE_ID,
      contact: "alice@example.com",
    },
  ],
  allegations: [{ code: "A1", summary: "ownership dispute", policy_reference: "policy-1" }],
  evidence: [
    {
      id: "evidence-1",
      kind: "document",
      uri: "s3://evidence/alice",
      hash: `0x${"aa".repeat(32)}`,
      description: "Initial report",
      sealed: true,
    },
  ],
  sla: {
    acknowledge_by: "2026-04-01T02:00:00Z",
    resolution_by: "2026-04-05T00:00:00Z",
    extensions: [
      {
        approved_by: "council",
        reason: "Awaiting guardian review",
        new_resolution_by: "2026-04-06T00:00:00Z",
      },
    ],
  },
  actions: [
    {
      timestamp: "2026-04-01T00:10:00Z",
      actor: "registrar",
      action: "intake",
      notes: "Initial capture",
    },
  ],
  decision: {
    finding: "upheld",
    remedies: ["refund"],
    effective_at: "2026-04-05T01:00:00Z",
    publication_state: "public",
  },
});

function sampleRbcSession(overrides = {}) {
  return {
    blockHash: "ab".repeat(32),
    height: 12,
    view: 3,
    totalChunks: 8,
    receivedChunks: 4,
    readyCount: 2,
    delivered: false,
    invalid: false,
    payloadHash: null,
    recovered: false,
    ...overrides,
  };
}

function accountPath(accountId, suffix) {
  const normalized = normalizeAccountId(accountId, "accountId");
  return `/v1/accounts/${encodeURIComponent(normalized)}${suffix}`;
}

function fakeHashHex(byte) {
  return Buffer.alloc(32, byte & 0xff).toString("hex");
}

function sampleVerifyingKeyRegisterPayload() {
  return {
    authority: SAMPLE_ACCOUNT_ID,
    private_key: "ed0120",
    backend: "halo2/ipa",
    name: "vk_main",
    version: 1,
    circuit_id: "halo2/ipa::transfer_v1",
    public_inputs_schema_hash_hex: "0xdeadbeef",
    gas_schedule_id: "default",
  };
}

function bufferToBase64Url(buffer) {
  return buffer
    .toString("base64")
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/u, "");
}

test("ToriiClient constructor enforces option shapes", () => {
  assert.throws(
    () => new ToriiClient(BASE_URL, "invalid"),
    /ToriiClient options must be a plain object/,
  );

  const fetchImpl = async () => createResponse({ status: 200, jsonData: {} });
  assert.throws(
    () =>
      new ToriiClient(BASE_URL, {
        fetchImpl,
        sorafsGatewayFetch: "not-a-function",
      }),
    /options\.sorafsGatewayFetch must be a function/,
  );
  assert.throws(
    () =>
      new ToriiClient(BASE_URL, {
        fetchImpl,
        generateDaProofSummary: 42,
      }),
    /options\.generateDaProofSummary must be a function/,
  );
  assert.throws(
    () =>
      new ToriiClient(BASE_URL, {
        fetchImpl,
        sorafsAliasPolicy: 7,
      }),
    /sorafsAliasPolicy must be a plain object/,
  );
  assert.throws(
    () =>
      new ToriiClient(BASE_URL, {
        fetchImpl,
        onSorafsAliasWarning: "not-a-hook",
      }),
    /onSorafsAliasWarning must be a function/,
  );
});

function createIsoSubmissionPayload(overrides = {}) {
  return {
    message_id: "iso-msg",
    status: "Accepted",
    pacs002_code: "ACSP",
    transaction_hash: null,
    hold_reason_code: null,
    change_reason_codes: [],
    rejection_reason_code: null,
    ledger_id: null,
    source_account_id: null,
    source_account_address: null,
    target_account_id: null,
    target_account_address: null,
    asset_definition_id: null,
    asset_id: null,
    ...overrides,
  };
}

function createIsoStatusPayload(overrides = {}) {
  return {
    ...createIsoSubmissionPayload(),
    detail: null,
    updated_at_ms: 1,
    ...overrides,
  };
}

function createPipelineRecoveryPayload(overrides = {}) {
  const baseTxs =
    overrides.txs ??
    [
      {
        hash: fakeHashHex(0x22),
        reads: ["World.accounts", "World.domains"],
        writes: ["World.accounts"],
      },
    ];
  return {
    format: "dag-json",
    height: 42,
    dag: {
      fingerprint: fakeHashHex(0xaa),
      key_count: 5,
      ...(overrides.dag ?? {}),
    },
    txs: baseTxs,
    ...overrides,
  };
}

function createOfflineTransferPayload(overrides = {}) {
  const controller = fixtureAccountForms("alice");
  const receiver = fixtureAccountForms("bob");
  const deposit = fixtureAccountForms("vault");
  const baseItem = {
    bundle_id_hex: "ff00",
    controller_id: controller.i105,
    controller_display: controller.i105Default,
    receiver_id: receiver.i105,
    receiver_display: receiver.i105Default,
    deposit_account_id: deposit.i105,
    deposit_account_display: deposit.i105Default,
    asset_id: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
    receipt_count: 1,
    total_amount: "1",
    claimed_delta: "1",
    status: "Settled",
    recorded_at_ms: 1_000,
    recorded_at_height: 5,
    archived_at_height: null,
    certificate_id_hex: "aa".repeat(32),
    certificate_expires_at_ms: 2_000,
    policy_expires_at_ms: 3_000,
    refresh_at_ms: 4_000,
    verdict_id_hex: "bb".repeat(32),
    attestation_nonce_hex: "cc".repeat(32),
    platform_policy: "provisioned",
    platform_token_snapshot: {
      policy: "provisioned",
      attestation_jws_b64: "eyJhbGciOiJFZERTQSJ9",
    },
    transfer: {
      receipts: [],
      balance_proof: { claimed_delta: "1" },
      metadata: {},
    },
    status_transitions: [],
  };
  const items = overrides.items ?? [baseItem];
  return {
    items,
    total: overrides.total ?? items.length,
  };
}

function createOfflineAllowanceItem(overrides = {}) {
  return {
    certificate_id_hex: "aa".repeat(32),
    controller_id: SAMPLE_ACCOUNT_ID,
    controller_display: SAMPLE_ACCOUNT_FORMS.i105Default,
    asset_id: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
    asset_definition_id: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
    asset_definition_name: "USD",
    asset_definition_alias: null,
    registered_at_ms: 1,
    expires_at_ms: 2,
    policy_expires_at_ms: 3,
    refresh_at_ms: null,
    remaining_amount: "10",
    record: { remaining_amount: "10" },
    ...overrides,
  };
}

function createOfflineSummaryItem(overrides = {}) {
  return {
    certificate_id_hex: "aa".repeat(32),
    controller_id: SAMPLE_ACCOUNT_ID,
    controller_display: SAMPLE_ACCOUNT_FORMS.i105Default,
    summary_hash_hex: "11".repeat(32),
    ...overrides,
  };
}

function createOfflineRevocationItem(overrides = {}) {
  return {
    verdict_id_hex: "bb".repeat(32),
    issuer_id: SAMPLE_ACCOUNT_ID,
    issuer_display: SAMPLE_ACCOUNT_FORMS.i105Default,
    revoked_at_ms: 10,
    reason: "certificate_revoked",
    note: null,
    record: {},
    ...overrides,
  };
}

test("listAccountAssets canonicalizes encoded account ids", async () => {
  const forms = sampleAccountForms();
  let capturedUrl;
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: { items: [], total: 0 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.listAccountAssets(forms.i105Default);
  assert.ok(
    capturedUrl?.includes(encodeURIComponent(forms.canonical)),
    `expected ${capturedUrl} to include canonical segment ${forms.canonical}`,
  );
});

test("listAccountAssets rejects Local-8 segments", async () => {
  const forms = sampleAccountForms();
  let called = false;
  const fetchImpl = async () => {
    called = true;
    return createResponse({ status: 200, jsonData: { items: [], total: 0 } });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listAccountAssets(forms.local8),
    (error) => {
      if (error instanceof ValidationError) {
        assert.equal(error.code, ValidationErrorCode.INVALID_ACCOUNT_ID);
        return true;
      }
      const isAccountAddressError = error instanceof AccountAddressError;
      const cause = error?.cause;
      const code = isAccountAddressError
        ? error.code
        : (cause instanceof AccountAddressError ? cause.code : null);
      assert(
        isAccountAddressError || error?.code === "ERR_INVALID_ACCOUNT_ID",
        `expected AccountAddressError or validation error, got ${error?.constructor?.name}`,
      );
      assert.ok(
        code === AccountAddressErrorCode.INVALID_LENGTH ||
          code === AccountAddressErrorCode.UNKNOWN_CONTROLLER_TAG ||
          code === AccountAddressErrorCode.LOCAL_DIGEST_TOO_SHORT,
        `unexpected error code ${code}`,
      );
      return true;
    },
  );
  assert.equal(called, false, "fetchImpl should not be invoked for invalid addresses");
});

test("uploadAttachment posts bytes with metadata", async () => {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url, init });
    return createResponse({
      status: 201,
      jsonData: {
        id: "abc",
        content_type: "text/plain",
        size: 5,
        created_ms: 123,
        tenant: null,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = Buffer.from("hello");
  const meta = await client.uploadAttachment(payload, { content_type: "text/plain" });
  assert.deepEqual(meta, {
    id: "abc",
    contentType: "text/plain",
    size: 5,
    createdMs: 123,
    tenant: null,
  });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, `${BASE_URL}/v1/zk/attachments`);
  assert.equal(calls[0].init.method, "POST");
  assert.equal(calls[0].init.headers["Content-Type"], "text/plain");
  assert.strictEqual(calls[0].init.body, payload);
});

test("uploadAttachment rejects malformed metadata responses", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 201,
        jsonData: { id: "", size: "oops" },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.uploadAttachment(Buffer.alloc(0), { contentType: "text/plain" }),
    /upload attachment response/,
  );
});

test("uploadAttachment rejects non-object metadata", async () => {
  let called = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      called = true;
      throw new Error("uploadAttachment should fail before fetching");
    },
  });
  await assert.rejects(
    () => client.uploadAttachment(Buffer.alloc(1), "text/plain"),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(error.message, /uploadAttachment options must be a plain object/);
      return true;
    },
  );
  assert.equal(called, false);
});

test("uploadAttachment requires a non-empty content type", async () => {
  let called = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      called = true;
      throw new Error("uploadAttachment should fail before fetching");
    },
  });
  await assert.rejects(
    () => client.uploadAttachment(Buffer.from("hi"), {}),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_STRING);
      assert.equal(error.message, "uploadAttachment options.contentType must be a string");
      return true;
    },
  );
  await assert.rejects(
    () => client.uploadAttachment(Buffer.from("hi"), { contentType: " " }),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_STRING);
      assert.equal(error.message, "uploadAttachment options.contentType must not be empty");
      return true;
    },
  );
  assert.equal(called, false);
});

test("uploadAttachment rejects unsupported payload types", async () => {
  let called = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      called = true;
      throw new Error("uploadAttachment should fail before fetching");
    },
  });
  await assert.rejects(
    () => client.uploadAttachment(42, { contentType: "application/octet-stream" }),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(error.message, /uploadAttachment data must be a string or binary payload/);
      return true;
    },
  );
  assert.equal(called, false);
});

test("listAttachments returns attachment metadata", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: [
        {
          id: "a",
          content_type: "application/json",
          size: 10,
          created_ms: 20,
          tenant: null,
        },
        {
          id: "b",
          content_type: "text/plain",
          size: 15,
          created_ms: 25,
          tenant: "tenant-1",
        },
      ],
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.listAttachments();
  assert.deepEqual(result, [
    {
      id: "a",
      contentType: "application/json",
      size: 10,
      createdMs: 20,
      tenant: null,
    },
    {
      id: "b",
      contentType: "text/plain",
      size: 15,
      createdMs: 25,
      tenant: "tenant-1",
    },
  ]);
});

test("listAttachments forwards AbortSignal", async () => {
  const controller = new AbortController();
  const fetchImpl = async (_url, init) => {
    assert.strictEqual(init.signal, controller.signal);
    return createResponse({
      status: 200,
      jsonData: [],
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.listAttachments({ signal: controller.signal });
  assert.deepEqual(result, []);
});

test("listAttachments rejects unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200, jsonData: [] }) });
  await assert.rejects(
    () => client.listAttachments({ signal: new AbortController().signal, extra: true }),
    /listAttachments options contains unsupported fields: extra/,
  );
});

test("listRepoAgreements normalizes repo payload", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: toriiFixtures.repo.list,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const page = await client.listRepoAgreements({ limit: 2 });
  assert.equal(page.total, 1);
  assert.equal(page.items.length, 1);
  const agreement = page.items[0];
  assert.equal(agreement.id, "alpha_repo");
  assert.equal(agreement.cashLeg.assetDefinitionId, "7EAD8EFYUx1aVKZPUU1fyKvr8dF1");
  assert.equal(agreement.collateralLeg.metadata.isin, "US0000000001");
  assert.equal(agreement.governance.marginFrequencySecs, 86400);
});

test("queryRepoAgreements posts structured envelope", async () => {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url, init });
    return createResponse({
      status: 200,
      jsonData: toriiFixtures.repo.list,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.queryRepoAgreements({ sort: "maturity_timestamp_ms:desc", limit: 1 });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, `${BASE_URL}/v1/repo/agreements/query`);
  assert.equal(calls[0].init.method, "POST");
  const body = JSON.parse(Buffer.from(calls[0].init.body).toString("utf8"));
  assert.ok(body.sort, "expected sort array in repo query body");
});

test("getAttachment validates attachmentId", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 200, arrayData: new ArrayBuffer(0) }),
  });
  await assert.rejects(() => client.getAttachment(""), /attachmentId/);
});

test("getAttachment returns bytes and content type", async () => {
  const data = new Uint8Array([1, 2, 3]).buffer;
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      arrayData: data,
      headers: { "content-type": "application/octet-stream" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getAttachment("att-1");
  assert.ok(Buffer.isBuffer(result.data));
  assert.deepEqual([...result.data.values()], [1, 2, 3]);
  assert.equal(result.contentType, "application/octet-stream");
});

test("getAttachment forwards AbortSignal", async () => {
  const controller = new AbortController();
  const fetchImpl = async (_url, init) => {
    assert.strictEqual(init.signal, controller.signal);
    return createResponse({
      status: 200,
      arrayData: new ArrayBuffer(0),
      headers: { "content-type": "application/octet-stream" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getAttachment("att-42", { signal: controller.signal });
  assert.ok(Buffer.isBuffer(result.data));
  assert.equal(result.contentType, "application/octet-stream");
});

test("getAttachment rejects unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 200, arrayData: new ArrayBuffer(0) }),
  });
  await assert.rejects(
    () => client.getAttachment("att-1", { signal: new AbortController().signal, extra: "nope" }),
    /getAttachment options contains unsupported fields: extra/,
  );
});

test("deleteAttachment issues delete request", async () => {
  let called = false;
  const fetchImpl = async (url, init) => {
    called = true;
    assert.equal(init.method, "DELETE");
    assert.equal(url, `${BASE_URL}/v1/zk/attachments/att-2`);
    return createResponse({ status: 202 });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.deleteAttachment("att-2");
  assert.ok(called);
});

test("deleteAttachment tolerates not found responses", async () => {
  const fetchImpl = async () => createResponse({ status: 404 });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.deleteAttachment("missing");
  await assert.rejects(() => client.deleteAttachment(""), /attachmentId/);
});

test("listVerifyingKeysTyped normalizes records", async () => {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url, init });
    return createResponse({
      status: 200,
      jsonData: {
        items: [
          {
            id: { backend: "halo2/ipa", name: "vk_main" },
            record: {
              version: 2,
              circuit_id: "halo2/ipa::transfer_v2",
              backend: "halo2/ipa",
              curve: "pallas",
              public_inputs_schema_hash: "deadbeef",
              commitment: "0x1234",
              vk_len: 4096,
              max_proof_bytes: 8192,
              gas_schedule_id: "halo2_default",
              metadata_uri_cid: null,
              vk_bytes_cid: "ipfs://vk",
              activation_height: 10,
              deprecation_height: 20,
              withdraw_height: 30,
              status: "active",
              key: {
                backend: "halo2/ipa",
                bytes_b64: Buffer.from("hello").toString("base64"),
              },
            },
          },
        ],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const list = await client.listVerifyingKeysTyped({
    backend: "halo2/ipa",
    status: "active",
    limit: 5,
    order: "asc",
  });

  assert.equal(list.length, 1);
  const entry = list[0];
  assert.deepEqual(entry.id, { backend: "halo2/ipa", name: "vk_main" });
  assert.equal(entry.record?.status, "Active");
  assert.equal(entry.record?.vk_len, 4096);
  assert.equal(entry.record?.inline_key?.backend, "halo2/ipa");
  assert.equal(entry.record?.inline_key?.bytes_b64, Buffer.from("hello").toString("base64"));

  const invoked = new URL(calls[0].url);
  assert.equal(invoked.pathname, "/v1/zk/vk");
  assert.equal(invoked.searchParams.get("backend"), "halo2/ipa");
  assert.equal(invoked.searchParams.get("status"), "Active");
  assert.equal(invoked.searchParams.get("limit"), "5");
  assert.equal(invoked.searchParams.get("order"), "asc");
});

test("iterateVerifyingKeys paginates and forwards filters", async () => {
  const seenOffsets = [];
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    const offset = Number(parsed.searchParams.get("offset") ?? "0");
    const limit = Number(parsed.searchParams.get("limit") ?? "0");
    seenOffsets.push(offset);
    assert.equal(parsed.searchParams.get("backend"), "halo2/ipa");
    assert.equal(parsed.searchParams.get("status"), "Active");
    assert.equal(limit, 1);
    if (offset >= 2) {
      throw new Error("unexpected extra verifier request");
    }
    return createResponse({
      status: 200,
      jsonData: {
        items: [{ id: { backend: "halo2/ipa", name: `vk-${offset}` } }],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const names = [];
  for await (const entry of client.iterateVerifyingKeys({
    backend: "halo2/ipa",
    status: "active",
    pageSize: 1,
    maxItems: 2,
  })) {
    names.push(entry.id.name);
  }
  assert.deepEqual(names, ["vk-0", "vk-1"]);
  assert.deepEqual(seenOffsets, [0, 1]);
});

test("iterateVerifyingKeys rejects unsupported iterator options", () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 200, jsonData: [], headers: { "content-type": "application/json" } }),
  });
  assert.throws(
    () => client.iterateVerifyingKeys({ backend: "halo2/ipa", extra: true }),
    /iterator options contains unsupported fields: extra/,
  );
});

test("listVerifyingKeys rejects unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("unexpected fetch");
    },
  });
  await assert.rejects(
    () => client.listVerifyingKeys({ backend: "halo2/ipa", extra: true }),
    /listVerifyingKeys options contains unsupported fields: extra/,
  );
});

test("listVerifyingKeys accepts alias option names", async () => {
  let captured;
  const fetchImpl = async (url) => {
    captured = url;
    return createResponse({
      status: 200,
      jsonData: [],
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.listVerifyingKeys({
    backend_filter: "halo2/ipa",
    statusFilter: "withdrawn",
    name_contains: "transfer",
    limit: 2,
    offset: 4,
    sortOrder: "DESC",
    ids_only: true,
  });

  assert.ok(captured);
  const params = new URL(captured).searchParams;
  assert.equal(params.get("backend"), "halo2/ipa");
  assert.equal(params.get("status"), "Withdrawn");
  assert.equal(params.get("name_contains"), "transfer");
  assert.equal(params.get("limit"), "2");
  assert.equal(params.get("offset"), "4");
  assert.equal(params.get("order"), "desc");
  assert.equal(params.get("ids_only"), "true");
});

test("getVerifyingKeyTyped decodes payload", async () => {
  const fetchImpl = async (url) => {
    assert.equal(url, `${BASE_URL}/v1/zk/vk/halo2%2Fipa/vk_main`);
    return createResponse({
      status: 200,
      jsonData: {
        id: { backend: "halo2/ipa", name: "vk_main" },
        record: {
          version: 1,
          circuit_id: "halo2/ipa::transfer_v1",
          backend: "halo2/ipa",
          curve: null,
          public_inputs_schema_hash: "abc123",
          commitment: "0xdead",
          vk_len: 1024,
          max_proof_bytes: 512,
          gas_schedule_id: null,
          metadata_uri_cid: null,
          vk_bytes_cid: null,
          activation_height: 5,
          withdraw_height: 7,
          status: "Proposed",
          key: null,
        },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const detail = await client.getVerifyingKeyTyped("halo2/ipa", "vk_main");
  assert.equal(detail.id.backend, "halo2/ipa");
  assert.equal(detail.id.name, "vk_main");
  assert.equal(detail.record.status, "Proposed");
  assert.equal(detail.record.vk_len, 1024);
  assert.equal(detail.record.inline_key, null);
});

test("registerVerifyingKey canonicalizes payload", async () => {
  let captured;
  const canonicalAuthority =
    FIXTURE_ALICE_ID;
  const fetchImpl = async (url, init) => {
    captured = { url, init, body: JSON.parse(init.body) };
    return createResponse({ status: 202 });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.registerVerifyingKey({
    authority: canonicalAuthority,
    private_key: "ed0120",
    backend: "halo2/ipa",
    name: "vk_main",
    version: 3,
    circuit_id: "halo2/ipa::transfer_v3",
    public_inputs_schema_hash_hex: "0xfeed",
    gas_schedule_id: "halo2_default",
    vk_bytes: Buffer.from("abc"),
    status: "withdrawn",
    activation_height: 0,
  });

  assert.ok(captured);
  assert.equal(captured.url, `${BASE_URL}/v1/zk/vk/register`);
  assert.equal(captured.init.method, "POST");
  assert.equal(
    captured.init.headers["Content-Type"],
    "application/json",
  );
  const body = captured.body;
  assert.equal(body.authority, normalizeAccountId(canonicalAuthority, "registerVerifyingKey.authority"));
  assert.equal(body.private_key, "ed0120");
  assert.equal(body.backend, "halo2/ipa");
  assert.equal(body.name, "vk_main");
  assert.equal(body.version, 3);
  assert.equal(body.circuit_id, "halo2/ipa::transfer_v3");
  assert.equal(body.public_inputs_schema_hash_hex, "0xfeed");
  assert.equal(body.gas_schedule_id, "halo2_default");
  assert.equal(body.vk_bytes, Buffer.from("abc").toString("base64"));
  assert.equal(body.vk_len, 3);
  assert.equal(body.status, "Withdrawn");
  assert.equal(body.activation_height, 0);
});

test("registerVerifyingKey rejects mismatched vk_len", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("unexpected fetch");
    },
  });
  const payload = {
    ...sampleVerifyingKeyRegisterPayload(),
    vk_bytes: Buffer.from("abc"),
    vk_len: 4,
  };
  await assert.rejects(
    () => client.registerVerifyingKey(payload),
    /vk_len/,
  );
});

test("verifying key endpoints reject unsupported option fields", async () => {
  const fetchImpl = async () => {
    throw new Error("unexpected fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getVerifyingKey("halo2/ipa", "vk_main", { extra: true }),
    /getVerifyingKey options contains unsupported fields: extra/,
  );
  const registerPayload = sampleVerifyingKeyRegisterPayload();
  await assert.rejects(
    () => client.registerVerifyingKey(registerPayload, { extra: "x" }),
    /registerVerifyingKey options contains unsupported fields: extra/,
  );
  await assert.rejects(
    () =>
      client.updateVerifyingKey(
        { ...registerPayload, status: "Active" },
        { extra: 123 },
      ),
    /updateVerifyingKey options contains unsupported fields: extra/,
  );
});

test("evaluateAliasVoprf posts JSON payload", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: { evaluated_element_hex: "ff", backend: "blake2b512-mock" },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.evaluateAliasVoprf("deadbeef");
  assert.equal(captured.url, `${BASE_URL}/v1/aliases/voprf/evaluate`);
  assert.equal(captured.init.headers["Content-Type"], "application/json");
  assert.deepEqual(JSON.parse(captured.init.body), { blinded_element_hex: "deadbeef" });
  assert.deepEqual(result, {
    evaluated_element_hex: "ff",
    backend: "blake2b512-mock",
  });
});

test("evaluateAliasVoprf rejects unexpected payloads", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { foo: "bar" },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.evaluateAliasVoprf("deadbeef"),
    /Unexpected alias VOPRF response payload/,
  );
});

const VALID_IBAN = "GB82WEST12345698765432";

test("resolveAlias returns null on 404 and throws on 503", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 404 }),
  });
  const unresolved = await client.resolveAlias(VALID_IBAN);
  assert.equal(unresolved, null);

  const failingClient = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 503 }),
  });
  await assert.rejects(() => failingClient.resolveAlias(VALID_IBAN), /ISO bridge runtime/);
});

test("resolveAliasByIndex posts numeric payload", async () => {
  let body;
  const fetchImpl = async () => {
    return createResponse({
      status: 200,
      jsonData: { alias: VALID_IBAN, account_id: FIXTURE_ALICE_ID },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url, init) => {
      body = JSON.parse(init.body);
      return fetchImpl(url, init);
    },
  });
  const resolved = await client.resolveAliasByIndex(0);
  assert.equal(body.index, 0);
  assert.equal(resolved?.account_id, FIXTURE_ALICE_ID);
});

test("resolveAlias normalizes payload fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          alias: " GB82WEST12345698765432 ",
          account_id: FIXTURE_ALICE_ID,
          index: "5",
          source: "iso_bridge",
        },
        headers: { "content-type": "application/json" },
      }),
  });
  const resolved = await client.resolveAlias(VALID_IBAN);
  assert.deepEqual(resolved, {
    alias: VALID_IBAN,
    account_id: FIXTURE_ALICE_ID,
    index: 5,
    source: "iso_bridge",
  });
});

test("resolveAlias canonicalizes IBANs returned by Torii", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          alias: "gb82 west12345698765432",
          account_id: FIXTURE_ALICE_ID,
        },
        headers: { "content-type": "application/json" },
      }),
  });
  const resolved = await client.resolveAlias(VALID_IBAN);
  assert.equal(resolved.alias, VALID_IBAN);
  assert.equal(resolved.account_id, FIXTURE_ALICE_ID);
});

test("resolveAlias normalizes IBAN input before issuing the request", async () => {
  let capturedBody;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url, init) => {
      capturedBody = JSON.parse(init.body);
      return createResponse({
        status: 200,
        jsonData: { alias: VALID_IBAN, account_id: FIXTURE_ALICE_ID },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await client.resolveAlias(" gb82 west12345698765432 ");
  assert.equal(capturedBody.alias, VALID_IBAN);
});

test("resolveAlias rejects IBANs that fail checksum validation", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("resolveAlias should have rejected before fetching");
    },
  });
  await assert.rejects(
    () => client.resolveAlias("GB00WEST12345698765432"),
    /mod-97/,
  );
});

test("resolveAlias rejects malformed payloads", async () => {
  const missingAliasClient = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          alias: "   ",
          account_id: FIXTURE_ALICE_ID,
        },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => missingAliasClient.resolveAlias(VALID_IBAN),
    /alias resolve response\.alias must not be empty/,
  );

  const invalidSourceClient = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          alias: VALID_IBAN,
          account_id: FIXTURE_ALICE_ID,
          source: 42,
        },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => invalidSourceClient.resolveAlias(VALID_IBAN),
    /alias resolve response\.source must be a string/,
  );
});

test("resolveAlias rejects responses with invalid IBAN aliases", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          alias: "GB00WEST12345698765432",
          account_id: FIXTURE_ALICE_ID,
        },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.resolveAlias(VALID_IBAN),
    /alias resolve response\.alias.*mod-97/,
  );
});

test("submitIsoPacs008 posts XML payload and returns submission metadata", async () => {
  let captured;
  const submissionPayload = createIsoSubmissionPayload({ message_id: "MSG123" });
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 202,
      jsonData: submissionPayload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const xmlPayload = "<pacs.008>ok</pacs.008>";
  const response = await client.submitIsoPacs008(xmlPayload);
  assert.equal(captured.url, `${BASE_URL}/v1/iso20022/pacs008`);
  assert.equal(captured.init.method, "POST");
  assert.equal(captured.init.headers["Content-Type"], "application/xml");
  assert.equal(captured.init.headers.Accept, "application/json");
  assert.ok(Buffer.isBuffer(captured.init.body));
  assert.equal(captured.init.body.toString("utf8"), xmlPayload);
  assert.deepEqual(response, submissionPayload);
});

test("submitIsoPacs008 enforces message payload", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 202 }) });
  await assert.rejects(
    () => client.submitIsoPacs008(null),
    (error) => expectValidationErrorFixture(error, "submitIsoPacs008_message_required"),
  );
});

test("submitIsoPacs008 rejects invalid AbortSignal option", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not submit");
    },
  });
  await assert.rejects(
    () =>
      client.submitIsoPacs008("<xml/>", {
        // @ts-expect-error runtime validation should reject incorrect signal
        signal: {},
      }),
    (error) => expectValidationErrorFixture(error, "submitIsoPacs008_invalid_signal"),
  );
});

test("submitIsoPacs008 rejects invalid contentType overrides", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not submit");
    },
  });
  await assert.rejects(
    () =>
      client.submitIsoPacs008("<xml/>", {
        // @ts-expect-error runtime validation should reject incorrect type
        contentType: {},
      }),
    (error) => expectValidationErrorFixture(error, "submitIsoPacs008_content_type_type"),
  );
  await assert.rejects(
    () =>
      client.submitIsoPacs008("<xml/>", {
        contentType: "   ",
      }),
    (error) => expectValidationErrorFixture(error, "submitIsoPacs008_content_type_empty"),
  );
});

test("submitIsoPacs008 forwards retryProfile to fetch options", async () => {
  let captured;
  const submissionPayload = createIsoSubmissionPayload({ message_id: "RP1" });
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should be mocked");
    },
  });
  client._request = async (_method, _url, init = {}) => {
    captured = init;
    return createResponse({
      status: 202,
      jsonData: submissionPayload,
      headers: { "content-type": "application/json" },
    });
  };
  await client.submitIsoPacs008("<xml/>", { retryProfile: "iso-bridge" });
  assert.equal(captured?.retryProfile, "iso-bridge");
});

test("submitIsoPacs008 rejects invalid retryProfile overrides", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not submit");
    },
  });
  await assert.rejects(
    () =>
      client.submitIsoPacs008("<xml/>", {
        // @ts-expect-error runtime validation should reject incorrect type
        retryProfile: 42,
      }),
    (error) => expectValidationErrorFixture(error, "submitIsoPacs008_retry_profile_type"),
  );
  await assert.rejects(
    () =>
      client.submitIsoPacs008("<xml/>", {
        retryProfile: "   ",
      }),
    (error) => expectValidationErrorFixture(error, "submitIsoPacs008_retry_profile_empty"),
  );
});

test("submitIsoPacs008 rejects unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not submit");
    },
  });
  await assert.rejects(
    () =>
      client.submitIsoPacs008("<xml/>", {
        contentType: "application/xml",
        extra: "nope",
      }),
    (error) => expectValidationErrorFixture(error, "submitIsoPacs008_unsupported_option"),
  );
});

test("submitIsoPacs009 forwards binary body, custom content type, and signal", async () => {
  let captured;
  const controller = new AbortController();
  const submissionPayload = createIsoSubmissionPayload({ message_id: "MSG777" });
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 202,
      jsonData: submissionPayload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const binaryPayload = new Uint8Array([0x3c, 0x70, 0x61, 0x63, 0x73, 0x2e, 0x30, 0x30, 0x39, 0x3e]);
  const response = await client.submitIsoPacs009(binaryPayload, {
    contentType: "application/pain+xml",
    signal: controller.signal,
  });
  assert.equal(captured.url, `${BASE_URL}/v1/iso20022/pacs009`);
  assert.equal(captured.init.headers["Content-Type"], "application/pain+xml");
  assert.equal(captured.init.headers.Accept, "application/json");
  assert.ok(captured.init.signal instanceof AbortSignal);
  assert.ok(Buffer.isBuffer(captured.init.body));
  assert.deepEqual([...captured.init.body.values()], [...binaryPayload.values()]);
  assert.deepEqual(response, submissionPayload);
});

test("submitIsoPacs009 enforces contentType overrides", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not submit");
    },
  });
  await assert.rejects(
    () =>
      client.submitIsoPacs009("<xml/>", {
        // @ts-expect-error runtime validation should reject incorrect type
        contentType: 42,
      }),
    (error) => expectValidationErrorFixture(error, "submitIsoPacs009_content_type_type"),
  );
  await assert.rejects(
    () =>
      client.submitIsoPacs009("<xml/>", {
        contentType: "",
      }),
    (error) => expectValidationErrorFixture(error, "submitIsoPacs009_content_type_empty"),
  );
});

test("submitIsoPacs009 rejects invalid retryProfile overrides", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not submit");
    },
  });
  await assert.rejects(
    () =>
      client.submitIsoPacs009("<xml/>", {
        // @ts-expect-error runtime validation should reject incorrect type
        retryProfile: {},
      }),
    (error) => expectValidationErrorFixture(error, "submitIsoPacs009_retry_profile_type"),
  );
  await assert.rejects(
    () =>
      client.submitIsoPacs009("<xml/>", {
        retryProfile: " ",
      }),
    (error) => expectValidationErrorFixture(error, "submitIsoPacs009_retry_profile_empty"),
  );
});

test("getIsoMessageStatus fetches status JSON and validates input", async () => {
  let captured;
  const statusPayload = createIsoStatusPayload({
    message_id: "MSG999",
    status: "Accepted",
    transaction_hash: "abc123",
  });
  const controller = new AbortController();
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: statusPayload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.getIsoMessageStatus("MSG999", {
    signal: controller.signal,
  });
  assert.equal(captured.url, `${BASE_URL}/v1/iso20022/status/MSG999`);
  assert.equal(captured.init.method, "GET");
  assert.equal(captured.init.headers.Accept, "application/json");
  assert.strictEqual(captured.init.signal, controller.signal);
  assert.deepEqual(payload, statusPayload);

  await assert.rejects(
    () => client.getIsoMessageStatus("   "),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(error.message, /messageId must not be empty/);
      return true;
    },
  );
});

test("getIsoMessageStatus rejects non-object options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not fetch");
    },
  });
  await assert.rejects(
    // @ts-expect-error exercising runtime validation
    () => client.getIsoMessageStatus("msg-1", 42),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(error.message, /getIsoMessageStatus\.options must be a plain object/);
      return true;
    },
  );
});

test("getIsoMessageStatus rejects unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not fetch");
    },
  });
  await assert.rejects(
    () => client.getIsoMessageStatus("msg-1", { extra: true }),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(
        error.message,
        /getIsoMessageStatus\.options contains unsupported fields: extra/,
      );
      return true;
    },
  );
});

test("getIsoMessageStatus normalizes ISO bridge status and pacs002 code", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: createIsoStatusPayload({
        message_id: "iso-normalize",
        status: "pending",
        pacs002_code: "pdng",
      }),
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.getIsoMessageStatus("iso-normalize");
  assert.equal(payload?.status, "Pending");
  assert.equal(payload?.pacs002_code, "PDNG");
});

test("getIsoMessageStatus rejects unknown ISO status values and pacs002 codes", async () => {
  const badStatusClient = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: createIsoStatusPayload({ status: "unknown-status" }),
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => badStatusClient.getIsoMessageStatus("iso-bad-status"),
    /must be one of Pending, Accepted, Rejected, Committed/,
  );

  const badPacsClient = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: createIsoStatusPayload({ pacs002_code: "xxxx" }),
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => badPacsClient.getIsoMessageStatus("iso-bad-pacs"),
    /pacs002_code must be one of ACTC, ACSP, ACSC, ACWC, PDNG, RJCT/,
  );
});

test("getSorafsPinManifest enforces alias proof policy", async (t) => {
  const native = requireSorafsNative(t);
  if (!native) {
    return;
  }
  const policy = native.sorafsAliasPolicyDefaults();
  const now = Math.floor(Date.now() / 1000);
  const fixture = native.sorafsAliasProofFixture({
    generatedAtUnix: now - 60,
    expiresAtUnix: now + 600,
  });
  const proof = fixture.proofB64;
  const evaluation = native.sorafsEvaluateAliasProof(proof, policy, now);

  let called = 0;
  const fetchImpl = async () => {
    called += 1;
    return createResponse({
      status: 200,
      jsonData: { digest_hex: "deadbeef" },
      headers: {
        "content-type": "application/json",
        "sora-proof": proof,
        "sora-name": fixture.alias,
        "sora-proof-status": evaluation.status_label,
      },
    });
  };

  const client = new ToriiClient(BASE_URL, {
    fetchImpl,
    sorafsAliasPolicy: policy,
  });
  const result = await client.getSorafsPinManifest("deadbeef");
  assert.equal(called, 1);
  assert.deepEqual(result, { digest_hex: "deadbeef" });
});

test("getSorafsPinManifest rejects stale alias proof", async (t) => {
  const native = requireSorafsNative(t);
  if (!native) {
    return;
  }
  const policy = native.sorafsAliasPolicyDefaults();
  const now = Math.floor(Date.now() / 1000);
  const fixture = native.sorafsAliasProofFixture({
    generatedAtUnix: now - 10_000,
    expiresAtUnix: now - 1,
  });
  const proof = fixture.proofB64;
  const evaluation = native.sorafsEvaluateAliasProof(proof, policy, now);

  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {},
        headers: {
          "content-type": "application/json",
          "sora-proof": proof,
          "sora-name": fixture.alias,
          "sora-proof-status": evaluation.status_label,
        },
      }),
    sorafsAliasPolicy: policy,
  });

  await assert.rejects(
    () => client.getSorafsPinManifest("deadbeef"),
    /alias proof/i,
  );
});

test("getSorafsPinManifest invokes warning hook for refresh-window proofs", async (t) => {
  const native = requireSorafsNative(t);
  if (!native) {
    return;
  }
  const policy = native.sorafsAliasPolicyDefaults();
  const now = Math.floor(Date.now() / 1000);
  const refreshStart = policy.positiveTtlSecs - policy.refreshWindowSecs;
  const fixture = native.sorafsAliasProofFixture({
    generatedAtUnix: now - (refreshStart + 10),
    expiresAtUnix: now + 600,
  });
  const proof = fixture.proofB64;
  const evaluation = native.sorafsEvaluateAliasProof(proof, policy, now);
  assert.equal(evaluation.state, "refresh_window");

  let warning = null;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { digest_hex: "deadbeef" },
        headers: {
          "content-type": "application/json",
          "sora-proof": proof,
          "sora-name": fixture.alias,
          "sora-proof-status": evaluation.status_label,
        },
      }),
    sorafsAliasPolicy: policy,
    onSorafsAliasWarning: (payload) => {
      warning = payload;
    },
  });

  const result = await client.getSorafsPinManifest("deadbeef");
  assert.deepEqual(result, { digest_hex: "deadbeef" });
  assert.ok(warning, "warning hook not invoked");
  assert.equal(warning?.alias, fixture.alias);
  assert.equal(warning?.evaluation?.state, "refresh_window");
});

test("getSorafsPinManifest returns null when Torii responds with 404", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 404,
      headers: { "content-type": "application/json" },
      jsonData: { code: "ERR_NOT_FOUND" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getSorafsPinManifest("deadbeef".repeat(4));
  assert.equal(result, null);
});

test("getSorafsPinManifestTyped rejects when Torii responds with 404", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 404,
      headers: { "content-type": "application/json" },
      jsonData: { code: "ERR_NOT_FOUND" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getSorafsPinManifestTyped("deadbeef".repeat(4)),
    /sorafs pin manifest endpoint returned 404/,
  );
});

test("registerSorafsPinManifest posts payload and returns JSON", async () => {
  const manifestHex = "a".repeat(64);
  const chunkHex = "b".repeat(64);
  const successorHex = "c".repeat(64);
  let captured = null;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: { status: "queued", manifest_digest_hex: manifestHex },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.registerSorafsPinManifest({
    authority: FIXTURE_ALICE_ID,
    privateKey: "ed25519:deadbeef",
    chunker: {
      profileId: 1,
      namespace: "sorafs",
      name: "sf1",
      semver: "1.0.0",
      multihashCode: 0,
    },
    pinPolicy: { minReplicas: 3, storageClass: "hot", retentionEpoch: 72 },
    manifestDigestHex: manifestHex,
    chunkDigestSha3_256Hex: chunkHex,
    submittedEpoch: 42,
    alias: { namespace: "docs", name: "main", proof: Buffer.from("alias-proof") },
    successorOfHex: successorHex,
  });
  assert.equal(captured?.url, `${BASE_URL}/v1/sorafs/pin/register`);
  assert.equal(captured?.init?.method, "POST");
  const body = JSON.parse(captured?.init?.body ?? "{}");
  assert.equal(body.authority, FIXTURE_ALICE_ID);
  assert.equal(body.chunker_profile_id, 1);
  assert.equal(body.pin_policy?.storage_class?.type, "Hot");
  assert.equal(body.chunk_digest_sha3_256_hex, chunkHex);
  assert.equal(
    body.alias?.proof_base64,
    Buffer.from("alias-proof").toString("base64"),
  );
  assert.equal(body.successor_of_hex, successorHex);
  assert.deepEqual(result, { status: "queued", manifest_digest_hex: manifestHex });
});

test("registerSorafsPinManifest validates storage class input", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: () => {
      throw new Error("fetch should not be called");
    },
  });
  await assert.rejects(
    () =>
      client.registerSorafsPinManifest({
        authority: FIXTURE_ALICE_ID,
        privateKey: "ed25519:deadbeef",
        chunker: {
          profileId: 1,
          namespace: "sorafs",
          name: "sf1",
          semver: "1.0.0",
        },
        pinPolicy: { minReplicas: 3, storageClass: "lava" },
        manifestDigestHex: "a".repeat(64),
        chunkDigestSha3_256Hex: "b".repeat(64),
        submittedEpoch: 1,
      }),
    /storageClass must be Hot, Warm, or Cold/i,
  );
});

test("registerSorafsPinManifestTyped normalizes response payloads", async () => {
  const manifestHex = "a".repeat(64);
  const successorHex = "b".repeat(64);
  const aliasB64 = Buffer.from("alias-proof").toString("base64");
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        manifestDigestHex: manifestHex.toUpperCase(),
        chunkerHandle: "sorafs.sf1@1.0.0",
        submittedEpoch: "42",
        alias: {
          namespace: "docs",
          name: "main",
          proof_base64: aliasB64,
        },
        successor_of_hex: successorHex,
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.registerSorafsPinManifestTyped({
    authority: FIXTURE_ALICE_ID,
    privateKey: "ed25519:deadbeef",
    chunker: {
      profileId: 1,
      namespace: "sorafs",
      name: "sf1",
      semver: "1.0.0",
      multihashCode: 0,
    },
    pinPolicy: { minReplicas: 3, storageClass: "hot", retentionEpoch: 72 },
    manifestDigestHex: manifestHex,
    chunkDigestSha3_256Hex: "c".repeat(64),
    submittedEpoch: 42,
  });
  assert.deepEqual(result, {
    manifest_digest_hex: manifestHex,
    chunker_handle: "sorafs.sf1@1.0.0",
    submitted_epoch: 42,
    alias: {
      namespace: "docs",
      name: "main",
      proof_base64: aliasB64,
    },
    successor_of_hex: successorHex,
  });
});

test("getSorafsPinManifestTyped normalizes manifest, aliases, and orders", async (t) => {
  const native = requireSorafsNative(t);
  if (!native) {
    return;
  }
  const policy = native.sorafsAliasPolicyDefaults();
  const now = Math.floor(Date.now() / 1000);
  const fixture = native.sorafsAliasProofFixture({
    generatedAtUnix: now - 120,
    expiresAtUnix: now + 600,
  });
  const proof = fixture.proofB64;
  const evaluation = native.sorafsEvaluateAliasProof(proof, policy, now);

  const manifestHex = "e".repeat(64);
  const parentHex = "f".repeat(64);
  const councilHex = "1".repeat(64);
  const aliasProof = Buffer.from("pin-alias").toString("base64");
  const manifestRecord = {
    digest_hex: manifestHex,
    chunker: {
      profile_id: 1,
      namespace: "sorafs",
      name: "sf1",
      semver: "1.0.0",
      multihash_code: 0,
    },
    chunk_digest_sha3_256_hex: "2".repeat(64),
    pin_policy: { min_replicas: 3 },
    submitted_by: FIXTURE_CAROL_ID,
    submitted_epoch: 42,
    status: { state: "approved", epoch: 45 },
    metadata: { note: "demo" },
    alias: { namespace: "docs", name: "main", proof_b64: aliasProof },
    successor_of_hex: parentHex,
    status_timestamp_unix: 123,
    governance_refs: [
      {
        cid: "cid-1",
        kind: "AliasRotate",
        effective_at: "2025-01-01T00:00:00Z",
        effective_at_unix: 1_700_000_000,
        targets: { alias: "docs/main", pin_digest_hex: manifestHex },
        signers: [FIXTURE_CAROL_ID],
      },
    ],
    council_envelope_digest_hex: councilHex,
    lineage: {
      successor_of_hex: parentHex,
      head_hex: manifestHex,
      depth_to_head: 0,
      is_head: true,
      superseded_by: null,
      immediate_successor: null,
      anomalies: [],
    },
  };
  const aliasRecord = {
    alias: "sora/docs",
    namespace: "sora",
    name: "docs",
    manifest_digest_hex: manifestHex,
    bound_by: FIXTURE_ALICE_ID,
    bound_epoch: 10,
    expiry_epoch: 99,
    proof_b64: Buffer.from("proof").toString("base64"),
    cache_state: "fresh",
    cache_rotation_due: false,
    cache_age_seconds: 12,
    cache_decision: "serve",
    cache_reasons: ["ttl_ok"],
    cache_evaluation: { decision: "serve" },
    lineage: { head_hex: manifestHex },
  };
  const providerHex = "d".repeat(64);
  const orderRecord = {
    order_id_hex: "c".repeat(64),
    manifest_digest_hex: manifestHex,
    issued_by: FIXTURE_BOB_ID,
    issued_epoch: 50,
    deadline_epoch: 80,
    status: { state: "pending" },
    canonical_order_b64: Buffer.from("order").toString("base64"),
    order: { order_id_hex: "c".repeat(64), policy_hash_hex: manifestHex },
    receipts: [
      {
        provider_hex: providerHex,
        status: "pending",
        timestamp: 123,
        por_sample_digest_hex: null,
      },
    ],
    providers: [providerHex],
  };

  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          attestation: { block_height: 1 },
          manifest: manifestRecord,
          aliases: [aliasRecord],
          replication_orders: [orderRecord],
        },
        headers: {
          "content-type": "application/json",
          "sora-proof": proof,
          "sora-name": fixture.alias,
          "sora-proof-status": evaluation.status_label,
        },
      }),
    sorafsAliasPolicy: policy,
  });

  const detail = await client.getSorafsPinManifestTyped(manifestHex);
  assert.equal(detail.manifest.digest_hex, manifestHex);
  assert.equal(detail.aliases.length, 1);
  assert.equal(detail.replication_orders.length, 1);
  assert.equal(detail.replication_orders[0].providers[0], providerHex);
  assert.equal(detail.attestation?.block_height, 1);
});

test("getSorafsPinManifestTyped rejects non-integer status timestamps", async () => {
  const manifestHex = "e".repeat(64);
  const manifestRecord = {
    digest_hex: manifestHex,
    chunker: {
      profile_id: 1,
      namespace: "sorafs",
      name: "sf1",
      semver: "1.0.0",
      multihash_code: 0,
    },
    chunk_digest_sha3_256_hex: "2".repeat(64),
    pin_policy: { min_replicas: 3 },
    submitted_by: FIXTURE_CAROL_ID,
    submitted_epoch: 42,
    status: { state: "approved", epoch: 45 },
    metadata: {},
    status_timestamp_unix: 123.5,
  };
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          attestation: null,
          manifest: manifestRecord,
          aliases: [],
          replication_orders: [],
        },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.getSorafsPinManifestTyped(manifestHex),
    (error) => {
      assert(error instanceof RangeError);
      assert.match(error.message, /status_timestamp_unix/);
      return true;
    },
  );
});

test("listSorafsAliases normalizes response and applies filters", async () => {
  let capturedUrl;
  const manifestHex = "a".repeat(64);
  const aliasRecord = {
    alias: "sora/docs",
    namespace: "sora",
    name: "docs",
    manifest_digest_hex: manifestHex,
    bound_by: FIXTURE_ALICE_ID,
    bound_epoch: 10,
    expiry_epoch: 99,
    proof_b64: Buffer.from("proof").toString("base64"),
    cache_state: "fresh",
    cache_rotation_due: false,
    cache_age_seconds: 12,
    cache_decision: "serve",
    cache_reasons: ["ttl_ok"],
    cache_evaluation: { decision: "serve" },
    lineage: { head_hex: manifestHex },
  };
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: {
        attestation: { block_height: 1 },
        total_count: 1,
        returned_count: 1,
        offset: 0,
        limit: 50,
        aliases: [aliasRecord],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.listSorafsAliases({
    namespace: "Sora",
    manifestDigestHex: `0x${manifestHex}`,
    limit: "5",
    offset: 10n,
  });
  assert.ok(capturedUrl?.startsWith(`${BASE_URL}/v1/sorafs/aliases`));
  const parsed = new URL(capturedUrl);
  assert.equal(parsed.searchParams.get("namespace"), "Sora");
  assert.equal(parsed.searchParams.get("manifest_digest"), manifestHex);
  assert.equal(parsed.searchParams.get("limit"), "5");
  assert.equal(parsed.searchParams.get("offset"), "10");
  assert.equal(result.aliases.length, 1);
  assert.equal(result.aliases[0].alias, aliasRecord.alias);
  assert.equal(result.aliases[0].cache_state, "fresh");
  assert.deepEqual(result.aliases[0].cache_reasons, aliasRecord.cache_reasons);
  assert.equal(result.attestation?.block_height, 1);
});

test("listSorafsPinManifests normalizes manifest payloads and guards filters", async () => {
  let capturedUrl;
  const manifestHex = "e".repeat(64);
  const parentHex = "f".repeat(64);
  const councilHex = "1".repeat(64);
  const aliasProof = Buffer.from("pin-alias").toString("base64");
  const manifestRecord = {
    digest_hex: manifestHex,
    chunker: {
      profile_id: 1,
      namespace: "sorafs",
      name: "sf1",
      semver: "1.0.0",
      multihash_code: 0,
    },
    chunk_digest_sha3_256_hex: "2".repeat(64),
    pin_policy: { min_replicas: 3 },
    submitted_by: FIXTURE_CAROL_ID,
    submitted_epoch: 42,
    status: { state: "approved", epoch: 45 },
    metadata: { note: "demo" },
    alias: { namespace: "docs", name: "main", proof_b64: aliasProof },
    successor_of_hex: parentHex,
    status_timestamp_unix: 123,
    governance_refs: [
      {
        cid: "cid-1",
        kind: "AliasRotate",
        effective_at: "2025-01-01T00:00:00Z",
        effective_at_unix: 1_700_000_000,
        targets: { alias: "docs/main", pin_digest_hex: manifestHex },
        signers: [FIXTURE_CAROL_ID],
      },
    ],
    council_envelope_digest_hex: councilHex,
    lineage: {
      successor_of_hex: parentHex,
      head_hex: manifestHex,
      depth_to_head: 0,
      is_head: true,
      superseded_by: null,
      immediate_successor: null,
      anomalies: [],
    },
  };
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: {
        attestation: { block_hash: "abc" },
        total_count: 1,
        returned_count: 1,
        offset: 0,
        limit: 10,
        manifests: [manifestRecord],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.listSorafsPinManifests({
    status: "APPROVED",
    limit: "10",
    offset: 0n,
  });
  assert.ok(capturedUrl?.startsWith(`${BASE_URL}/v1/sorafs/pin`));
  const parsed = new URL(capturedUrl);
  assert.equal(parsed.searchParams.get("status"), "approved");
  assert.equal(parsed.searchParams.get("limit"), "10");
  assert.equal(parsed.searchParams.get("offset"), "0");
  assert.equal(result.manifests.length, 1);
  const manifest = result.manifests[0];
  assert.equal(manifest.digest_hex, manifestHex);
  assert.equal(manifest.chunker.name, "sf1");
  assert.equal(manifest.status.state, "approved");
  assert.equal(manifest.alias?.proof_b64, aliasProof);
  assert.equal(manifest.lineage?.is_head, true);
  assert.equal(manifest.governance_refs[0]?.targets.alias, "docs/main");
  await assert.rejects(
    () => client.listSorafsPinManifests({ status: "invalid" }),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_STRING);
      assert.equal(error.path, "sorafsPinList.status");
      assert.match(error.message, /sorafsPinList\.status/);
      return true;
    },
  );
});

test("listSorafsReplicationOrders normalizes response and validates status filter", async () => {
  let capturedUrl;
  const manifestHex = "b".repeat(64);
  const orderHex = "c".repeat(64);
  const providerHex = "d".repeat(64);
  const orderRecord = {
    order_id_hex: orderHex,
    manifest_digest_hex: manifestHex,
    issued_by: FIXTURE_BOB_ID,
    issued_epoch: 50,
    deadline_epoch: 80,
    status: { state: "pending" },
    canonical_order_b64: Buffer.from("order").toString("base64"),
    order: { order_id_hex: orderHex, policy_hash_hex: manifestHex },
    receipts: [
      {
        provider_hex: providerHex,
        status: "pending",
        timestamp: 123,
        por_sample_digest_hex: null,
      },
    ],
    providers: [providerHex],
  };
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: {
        attestation: null,
        total_count: 1,
        returned_count: 1,
        offset: 0,
        limit: 20,
        replication_orders: [orderRecord],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.listSorafsReplicationOrders({
    status: "Pending",
    manifestDigestHex: manifestHex,
    limit: 20,
  });
  assert.ok(capturedUrl?.startsWith(`${BASE_URL}/v1/sorafs/replication`));
  const parsed = new URL(capturedUrl);
  assert.equal(parsed.searchParams.get("status"), "pending");
  assert.equal(parsed.searchParams.get("manifest_digest"), manifestHex);
  assert.equal(parsed.searchParams.get("limit"), "20");
  assert.equal(result.replication_orders[0].order_id_hex, orderHex);
  assert.equal(result.replication_orders[0].receipts[0].provider_hex, providerHex);
  await assert.rejects(
    () => client.listSorafsReplicationOrders({ status: "finished" }),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_STRING);
      assert.equal(error.path, "sorafsReplicationList.status");
      assert.match(error.message, /sorafsReplicationList\.status/i);
      return true;
    },
  );
});

test("getUaidPortfolio normalizes UAID literals and dataspace payloads", async () => {
  let capturedUrl;
  const fixture = cloneFixture(toriiFixtures.uaid.portfolio);
  fixture.dataspaces[0].accounts[0].account_id = FIXTURE_ALICE_ID;
  fixture.dataspaces[0].accounts[0].assets[0].asset_id = FIXTURE_ASSET_ID_A;
  fixture.dataspaces[1].accounts[0].account_id = FIXTURE_BOB_ID;
  fixture.dataspaces[1].accounts[0].assets[0].asset_id = FIXTURE_ASSET_ID_B;
  fixture.dataspaces[1].accounts[0].assets[1].asset_id = FIXTURE_ASSET_ID_C;
  const canonical = fixture.uaid;
  const rawHex = canonical.slice("uaid:".length);
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: fixture,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getUaidPortfolio(rawHex);
  assert.equal(
    capturedUrl,
    `${BASE_URL}/v1/accounts/${encodeURIComponent(canonical)}/portfolio`,
  );
  assert.equal(result.uaid, canonical);
  assert.equal(result.totals.accounts, 2);
  assert.equal(result.dataspaces.length, 2);
  assert.equal(result.dataspaces[1].accounts[0].assets[1].asset_definition_id, "fx#cbdc");
  await assert.rejects(() => client.getUaidPortfolio("short"), /uaid/);
});

test("getUaidPortfolio accepts mixed-case UAID prefixes", async () => {
  let capturedUrl;
  const fixture = cloneFixture(toriiFixtures.uaid.portfolio);
  fixture.dataspaces[0].accounts[0].account_id = FIXTURE_ALICE_ID;
  fixture.dataspaces[0].accounts[0].assets[0].asset_id = FIXTURE_ASSET_ID_A;
  fixture.dataspaces[1].accounts[0].account_id = FIXTURE_BOB_ID;
  fixture.dataspaces[1].accounts[0].assets[0].asset_id = FIXTURE_ASSET_ID_B;
  fixture.dataspaces[1].accounts[0].assets[1].asset_id = FIXTURE_ASSET_ID_C;
  const canonical = fixture.uaid;
  const rawHex = canonical.slice("uaid:".length);
  const mixed = `UaiD:  ${rawHex.toUpperCase()}  `;
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: fixture,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getUaidPortfolio(` ${mixed} `);
  assert.equal(
    capturedUrl,
    `${BASE_URL}/v1/accounts/${encodeURIComponent(canonical)}/portfolio`,
  );
  assert.equal(result.uaid, canonical);
});

test("getUaidPortfolio encodes assetId filters", async () => {
  let capturedUrl;
  const fixture = cloneFixture(toriiFixtures.uaid.portfolio);
  fixture.dataspaces[0].accounts[0].account_id = FIXTURE_ALICE_ID;
  fixture.dataspaces[0].accounts[0].assets[0].asset_id = FIXTURE_ASSET_ID_A;
  fixture.dataspaces[1].accounts[0].account_id = FIXTURE_BOB_ID;
  fixture.dataspaces[1].accounts[0].assets[0].asset_id = FIXTURE_ASSET_ID_B;
  fixture.dataspaces[1].accounts[0].assets[1].asset_id = FIXTURE_ASSET_ID_C;
  const assetId = fixture.dataspaces[0].accounts[0].assets[0].asset_id;
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: fixture,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.getUaidPortfolio(fixture.uaid, { assetId });
  const parsed = new URL(capturedUrl);
  assert.equal(parsed.pathname, `/v1/accounts/${encodeURIComponent(fixture.uaid)}/portfolio`);
  assert.equal(parsed.searchParams.get("asset_id"), assetId);
});

test("getUaidPortfolio rejects unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run for option validation");
    },
  });
  await assert.rejects(
    () => client.getUaidPortfolio(toriiFixtures.uaid.portfolio.uaid, { retry: true }),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_OBJECT);
      assert.equal(error.path, "getUaidPortfolio.options");
      assert.match(
        error.message,
        /getUaidPortfolio options contains unsupported fields: retry/,
      );
      return true;
    },
  );
});

test("getUaidBindings enforces UAID formats and normalizes entries", async () => {
  let capturedUrl;
  const fixture = cloneFixture(toriiFixtures.uaid.bindings);
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: fixture,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getUaidBindings(fixture.uaid);
  const parsed = new URL(capturedUrl);
  assert.equal(parsed.origin + parsed.pathname, `${BASE_URL}/v1/space-directory/uaids/${encodeURIComponent(fixture.uaid)}`);
  assert.equal(parsed.search, "");
  assert.equal(result.dataspaces[0].accounts[0], fixture.dataspaces[0].accounts[0]);
  await assert.rejects(() => client.getUaidBindings("uaid:xyz"), /64 hex characters/);
  await assert.rejects(
    () => client.getUaidBindings(`uaid:${"10".repeat(32)}`),
    /least significant bit/i,
  );
  await assert.rejects(
    () => client.getUaidBindings(fixture.uaid, { legacyFormat: "i105" }),
    /getUaidBindings options contains unsupported fields: legacyFormat/,
  );
});

test("getUaidManifests validates lifecycle metadata and filters by dataspace", async () => {
  let capturedUrl;
  const fixture = cloneFixture(toriiFixtures.uaid.manifests);
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: fixture,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getUaidManifests(fixture.uaid, { dataspaceId: 11 });
  const parsed = new URL(capturedUrl);
  assert.equal(parsed.searchParams.get("dataspace"), "11");
  assert.equal(parsed.searchParams.get("canonical_i105"), null);
  assert.equal(result.manifests.length, 1);
  const record = result.manifests[0];
  assert.equal(record.status, "Active");
  assert.equal(record.lifecycle.activated_epoch, 4097);
  assert.equal(record.manifest.entries[0].effect.Allow.max_amount, "500000000");
  await assert.rejects(
    () => client.getUaidManifests(fixture.uaid, { dataspaceId: 11, legacyFormat: "i105" }),
    /getUaidManifests options contains unsupported fields: legacyFormat/,
  );
});

test("publishSpaceDirectoryManifest posts manifest payloads with normalized keys", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 202,
      headers: { "content-type": "application/json" },
      jsonData: {},
    });
  };
  const manifest = cloneFixture(toriiFixtures.uaid.manifests.manifests[0].manifest);
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const privateKeyHex = "11".repeat(32);
  await client.publishSpaceDirectoryManifest({
    authority: FIXTURE_AUTHORITY_ID,
    manifest,
    privateKeyHex,
    reason: "rotation audit",
  });
  assert.equal(captured.url, `${BASE_URL}/v1/space-directory/manifests`);
  assert.equal(captured.init.method, "POST");
  const parsedBody = JSON.parse(captured.init.body.toString());
  assert.equal(parsedBody.authority, FIXTURE_AUTHORITY_ID);
  assert.equal(parsedBody.reason, "rotation audit");
  assert.deepEqual(parsedBody.manifest, manifest);
  assert.equal(parsedBody.private_key, `ed25519:${privateKeyHex}`);
});

test("publishSpaceDirectoryManifest canonicalizes manifest payloads", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 202,
      headers: { "content-type": "application/json" },
      jsonData: {},
    });
  };
  const manifestInput = {
    Version: "V1",
    uaidLiteral: toriiFixtures.uaid.manifests.uaid.toUpperCase(),
    dataspaceId: 7,
    issuedMs: "2048",
    activationEpoch: "512",
    expiryEpoch: 4096,
    accounts: [FIXTURE_ALICE_ID],
    Entries: [
      {
        scope: { program: "demo.transfer" },
        effect: { Allow: { max_amount: "500000000", window: "PerDay" } },
        notes: "demo rotation manifest",
      },
    ],
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.publishSpaceDirectoryManifest({
    authority: FIXTURE_AUTHORITY_ID,
    privateKeyHex: "22".repeat(32),
    manifest: manifestInput,
  });
  const parsed = JSON.parse(captured.init.body.toString());
  assert.equal(captured.url, `${BASE_URL}/v1/space-directory/manifests`);
  const manifest = parsed.manifest;
  assert.equal(manifest.version, "V1");
  assert.equal(
    manifest.uaid,
    toriiFixtures.uaid.manifests.uaid.toLowerCase(),
  );
  assert.equal(manifest.dataspace, 7);
  assert.equal(manifest.issued_ms, 2048);
  assert.equal(manifest.activation_epoch, 512);
  assert.equal(manifest.expiry_epoch, 4096);
  assert.deepEqual(manifest.accounts, [FIXTURE_ALICE_ID]);
  assert.equal(manifest.entries.length, 1);
  assert.deepEqual(
    manifest.entries[0].effect,
    manifestInput.Entries[0].effect,
  );
});

test("publishSpaceDirectoryManifest forwards AbortSignal options", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 202,
      headers: { "content-type": "application/json" },
      jsonData: {},
    });
  };
  const controller = new AbortController();
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.publishSpaceDirectoryManifest(
    {
      authority: FIXTURE_AUTHORITY_ID,
      privateKeyHex: "77".repeat(32),
      manifest: toriiFixtures.uaid.manifests.manifests[0].manifest,
    },
    { signal: controller.signal },
  );
  assert.equal(captured.init.signal, controller.signal);
});

test("publishSpaceDirectoryManifest rejects invalid options payloads", async () => {
  const fetchImpl = async () => {
    throw new Error("fetch should not run for invalid options");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () =>
      client.publishSpaceDirectoryManifest(
        {
          authority: FIXTURE_AUTHORITY_ID,
          privateKeyHex: "44".repeat(32),
          manifest: toriiFixtures.uaid.manifests.manifests[0].manifest,
        },
        /** @ts-expect-error */ 42,
      ),
    /publishSpaceDirectoryManifest options must be an object/,
  );
});

test("publishSpaceDirectoryManifest rejects invalid manifest entries", async () => {
  const fetchImpl = async () => {
    throw new Error("publishSpaceDirectoryManifest should not perform fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const uaid = toriiFixtures.uaid.manifests.uaid;
  await assert.rejects(
    () =>
      client.publishSpaceDirectoryManifest({
        authority: FIXTURE_AUTHORITY_ID,
        privateKeyHex: "12".repeat(32),
        manifest: {
          version: "V1",
          uaid,
          dataspace: 1,
          entries: [],
        },
      }),
    /entries must be a non-empty array/,
  );
  await assert.rejects(
    () =>
      client.publishSpaceDirectoryManifest({
        authority: FIXTURE_AUTHORITY_ID,
        privateKeyHex: "12".repeat(32),
        manifest: {
          version: "V1",
          uaid,
          dataspace: 1,
          entries: [{ scope: "demo", effect: null }],
        },
      }),
    /effect must be an object/,
  );
  await assert.rejects(
    () =>
      client.publishSpaceDirectoryManifest({
        authority: FIXTURE_AUTHORITY_ID,
        privateKeyHex: "12".repeat(32),
        manifest: {
          version: "V1",
          uaid,
          dataspace: 1,
          entries: [
            {
              scope: { program: "demo.transfer" },
              effect: { Allow: { max_amount: "1" } },
              notes: 123,
            },
          ],
        },
      }),
    /notes must be a string/,
  );
});

test("revokeSpaceDirectoryManifest normalizes UAIDs, epochs, and private key bytes", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 202,
      headers: { "content-type": "application/json" },
      jsonData: {},
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const seed = Buffer.alloc(32, 0x22);
  await client.revokeSpaceDirectoryManifest({
    authority: FIXTURE_AUTHORITY_ID,
    privateKey: seed,
    uaid: "0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
    dataspaceId: 11,
    revokedEpoch: 4096,
  });
  assert.equal(
    captured.url,
    `${BASE_URL}/v1/space-directory/manifests/revoke`,
  );
  const parsedBody = JSON.parse(captured.init.body.toString());
  assert.equal(parsedBody.dataspace, 11);
  assert.equal(parsedBody.revoked_epoch, 4096);
  assert.equal(
    parsedBody.uaid,
    "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  );
  assert.equal(parsedBody.private_key, `ed25519:${seed.toString("hex")}`);
});

test("revokeSpaceDirectoryManifest supports AbortSignal options", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 202,
      headers: { "content-type": "application/json" },
      jsonData: {},
    });
  };
  const controller = new AbortController();
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.revokeSpaceDirectoryManifest(
    {
      authority: FIXTURE_AUTHORITY_ID,
      privateKeyHex: "18".repeat(32),
      uaid: toriiFixtures.uaid.manifests.uaid,
      dataspaceId: 3,
      revokedEpoch: 512,
    },
    { signal: controller.signal },
  );
  assert.equal(captured.init.signal, controller.signal);
});

test("revokeSpaceDirectoryManifest rejects unsupported option fields", async () => {
  const fetchImpl = async () => {
    throw new Error("fetch should not run for invalid options");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () =>
      client.revokeSpaceDirectoryManifest(
        {
          authority: FIXTURE_AUTHORITY_ID,
          privateKeyHex: "19".repeat(32),
          uaid: toriiFixtures.uaid.manifests.uaid,
          dataspaceId: 5,
          revokedEpoch: 256,
        },
        { signal: new AbortController().signal, extra: "nope" },
      ),
    /revokeSpaceDirectoryManifest options contains unsupported fields: extra/,
  );
});

test("iterateSorafsAliases paginates alias listings", async () => {
  const baseAliasRecord = {
    alias: "sora/docs",
    namespace: "sora",
    name: "docs",
    manifest_digest_hex: "0".repeat(64),
    bound_by: FIXTURE_ALICE_ID,
    bound_epoch: 10,
    expiry_epoch: 99,
    proof_b64: Buffer.from("proof").toString("base64"),
    cache_state: "fresh",
    status_label: "ok",
    cache_rotation_due: false,
    cache_age_seconds: 12,
    proof_generated_at_unix: 1,
    proof_expires_at_unix: 2,
    proof_expires_in_seconds: 1,
    policy_positive_ttl_secs: 60,
    policy_refresh_window_secs: 30,
    policy_hard_expiry_secs: 120,
    policy_rotation_max_age_secs: 600,
    policy_successor_grace_secs: 10,
    policy_governance_grace_secs: 5,
    cache_decision: "serve",
    cache_reasons: ["ttl_ok"],
    cache_evaluation: { decision: "serve" },
    lineage: { head_hex: "0".repeat(64) },
  };
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    const offset = Number(parsed.searchParams.get("offset") ?? "0");
    const limit = Number(parsed.searchParams.get("limit") ?? "0");
    if (offset >= 2) {
      return createResponse({
        status: 200,
        jsonData: {
          attestation: null,
          total_count: 2,
          returned_count: 0,
          offset,
          limit,
          aliases: [],
        },
        headers: { "content-type": "application/json" },
      });
    }
    const record = {
      ...JSON.parse(JSON.stringify(baseAliasRecord)),
      alias: `sora/docs-${offset}`,
      name: `docs-${offset}`,
      manifest_digest_hex: `${offset}`.repeat(64),
    };
    return createResponse({
      status: 200,
      jsonData: {
        attestation: null,
        total_count: 2,
        returned_count: 1,
        offset,
        limit,
        aliases: [record],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const seen = [];
  for await (const alias of client.iterateSorafsAliases({ namespace: "sora", pageSize: 1 })) {
    seen.push(alias.alias);
  }
  assert.deepEqual(seen, ["sora/docs-0", "sora/docs-1"]);
});

test("iterateSorafsPinManifests honors maxItems and pagination", async () => {
  const baseManifest = {
    digest_hex: "e".repeat(64),
    chunker: {
      profile_id: 1,
      namespace: "sorafs",
      name: "sf1",
      semver: "1.0.0",
      multihash_code: 0,
    },
    chunk_digest_sha3_256_hex: "2".repeat(64),
    pin_policy: { min_replicas: 3 },
    submitted_by: FIXTURE_CAROL_ID,
    submitted_epoch: 42,
    status: { state: "approved", epoch: 45 },
    metadata: { note: "demo" },
    alias: { namespace: "docs", name: "main", proof_b64: Buffer.from("pin").toString("base64") },
    successor_of_hex: null,
    status_timestamp_unix: 123,
    governance_refs: [],
    council_envelope_digest_hex: "1".repeat(64),
    lineage: {
      successor_of_hex: null,
      head_hex: "e".repeat(64),
      depth_to_head: 0,
      is_head: true,
      superseded_by: null,
      immediate_successor: null,
      anomalies: [],
    },
  };
  let requestCount = 0;
  const fetchImpl = async (url) => {
    requestCount += 1;
    const parsed = new URL(url);
    const offset = Number(parsed.searchParams.get("offset") ?? "0");
    const limit = Number(parsed.searchParams.get("limit") ?? "0");
    if (offset >= 2) {
      return createResponse({
        status: 200,
        jsonData: {
          attestation: null,
          total_count: 2,
          returned_count: 0,
          offset,
          limit,
          manifests: [],
        },
        headers: { "content-type": "application/json" },
      });
    }
    const record = {
      ...JSON.parse(JSON.stringify(baseManifest)),
      digest_hex: `${offset}`.repeat(64),
      lineage: {
        ...JSON.parse(JSON.stringify(baseManifest.lineage)),
        head_hex: `${offset}`.repeat(64),
      },
    };
    return createResponse({
      status: 200,
      jsonData: {
        attestation: null,
        total_count: 2,
        returned_count: 1,
        offset,
        limit,
        manifests: [record],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const digests = [];
  for await (const manifest of client.iterateSorafsPinManifests({
    status: "approved",
    pageSize: 1,
    maxItems: 1,
  })) {
    digests.push(manifest.digest_hex);
  }
  assert.deepEqual(digests, ["0".repeat(64)]);
  assert.equal(requestCount, 1);
});

test("iterateSorafsReplicationOrders paginates results", async () => {
  const baseOrder = {
    order_id_hex: "c".repeat(64),
    manifest_digest_hex: "b".repeat(64),
    issued_by: FIXTURE_BOB_ID,
    issued_epoch: 50,
    deadline_epoch: 80,
    status: { state: "pending", epoch: null },
    canonical_order_b64: Buffer.from("order").toString("base64"),
    order: { order_id_hex: "c".repeat(64) },
    receipts: [
      {
        provider_hex: "d".repeat(64),
        status: "pending",
        timestamp: 123,
        por_sample_digest_hex: null,
      },
    ],
    providers: ["d".repeat(64)],
  };
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    const offset = Number(parsed.searchParams.get("offset") ?? "0");
    const limit = Number(parsed.searchParams.get("limit") ?? "0");
    if (offset >= 2) {
      return createResponse({
        status: 200,
        jsonData: {
          attestation: null,
          total_count: 2,
          returned_count: 0,
          offset,
          limit,
          replication_orders: [],
        },
        headers: { "content-type": "application/json" },
      });
    }
    const record = {
      ...JSON.parse(JSON.stringify(baseOrder)),
      order_id_hex: `${offset}`.repeat(64),
    };
    return createResponse({
      status: 200,
      jsonData: {
        attestation: null,
        total_count: 2,
        returned_count: 1,
        offset,
        limit,
        replication_orders: [record],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const ids = [];
  for await (const order of client.iterateSorafsReplicationOrders({
    status: "pending",
    pageSize: 1,
  })) {
    ids.push(order.order_id_hex);
  }
  assert.deepEqual(ids, ["0".repeat(64), "1".repeat(64)]);
});

test("SoraFS iterators reject unsupported options", () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("unexpected fetch");
    },
  });
  assert.throws(
    () =>
      client.iterateSorafsAliases({
        namespace: "sora",
        platformPolicy: "strict",
      }),
    /iterator options contains unsupported fields: platformPolicy/,
  );
  assert.throws(
    () =>
      client.iterateSorafsPinManifests({
        status: "approved",
        query: "ignored",
      }),
    /iterator options contains unsupported fields: query/,
  );
  assert.throws(
    () =>
      client.iterateSorafsReplicationOrders({
        status: "pending",
        filter: "noop",
      }),
    /iterator options contains unsupported fields: filter/,
  );
});

test("_iterateOffsetIterable enforces item-key whitelists", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 204 }),
  });
  const iterator = client._iterateOffsetIterable(
    async () => ({ manifests: ["m1", "m2"] }),
    {},
    new Set(["limit"]),
    ["manifests"],
  );
  const seen = [];
  for await (const manifest of iterator) {
    seen.push(manifest);
  }
  assert.deepEqual(seen, ["m1", "m2"]);

  const failingIterator = client._iterateOffsetIterable(
    async () => ({ manifests: ["m1"] }),
    {},
    new Set(["limit"]),
    ["aliases"],
  );
  await assert.rejects(
    async () => {
      await failingIterator.next();
    },
    /offset iterator response is missing iterable items \(expected: aliases\)/,
  );
});

test("pinSorafsManifest posts manifest and payload bytes", async () => {
  let captured = null;
  const manifestBytes = Buffer.from("norito-manifest");
  const payloadBytes = Buffer.from([0, 1, 2, 3]);
  const manifestHex = "a".repeat(64);
  const digestHex = "b".repeat(64);
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: {
        manifest_id_hex: manifestHex,
        payload_digest_hex: digestHex,
        content_length: payloadBytes.length,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.pinSorafsManifest({
    manifest: manifestBytes,
    payload: payloadBytes,
  });
  assert.equal(captured?.url, `${BASE_URL}/v1/sorafs/storage/pin`);
  assert.equal(captured?.init?.method, "POST");
  const body = JSON.parse(captured?.init?.body ?? "{}");
  assert.equal(body.manifest_b64, manifestBytes.toString("base64"));
  assert.equal(body.payload_b64, payloadBytes.toString("base64"));
  assert.deepEqual(result, {
    manifest_id_hex: manifestHex,
    payload_digest_hex: digestHex,
    content_length: payloadBytes.length,
  });
});

test("fetchSorafsPayloadRange normalizes request and response payloads", async () => {
  let captured = null;
  const manifestHex = "c".repeat(64);
  const providerBytes = Buffer.alloc(32, 0xaa);
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: {
        manifest_id_hex: manifestHex,
        offset: 4,
        length: 2,
        data_b64: Buffer.from([9, 9]).toString("base64"),
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.fetchSorafsPayloadRange({
    manifestIdHex: manifestHex,
    offset: 4,
    length: 2,
    providerIdHex: providerBytes,
  });
  assert.equal(captured?.url, `${BASE_URL}/v1/sorafs/storage/fetch`);
  const body = JSON.parse(captured?.init?.body ?? "{}");
  assert.equal(body.manifest_id_hex, manifestHex);
  assert.equal(body.offset, 4);
  assert.equal(body.length, 2);
  assert.equal(body.provider_id_hex, providerBytes.toString("hex"));
  assert.deepEqual(result, {
    manifest_id_hex: manifestHex,
    offset: 4,
    length: 2,
    data_b64: Buffer.from([9, 9]).toString("base64"),
  });
});

test("getSorafsStorageState returns typed fields", async () => {
  const snapshot = {
    bytes_used: 10,
    bytes_capacity: 100,
    pin_queue_depth: 1,
    fetch_inflight: 2,
    fetch_bytes_per_sec: 4096,
    por_inflight: 3,
    por_samples_success_total: 12,
    por_samples_failed_total: 1,
    fetch_utilisation_bps: 5000,
    pin_queue_utilisation_bps: 3000,
    por_utilisation_bps: 2000,
  };
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: snapshot,
        headers: { "content-type": "application/json" },
      }),
  });
  const result = await client.getSorafsStorageState();
  assert.deepEqual(result, snapshot);
});

test("getSorafsManifest normalizes response payload", async () => {
  const manifestHex = "d".repeat(64);
  const payloadDigestHex = "e".repeat(64);
  const manifestDigestHex = "f".repeat(64);
  const manifestB64 = Buffer.from("manifest").toString("base64");
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          manifest_id_hex: manifestHex,
          manifest_b64: manifestB64,
          manifest_digest_hex: manifestDigestHex,
          payload_digest_hex: payloadDigestHex,
          content_length: 42,
          chunk_count: 4,
          chunk_profile_handle: "profile@v1",
          stored_at_unix_secs: 123,
        },
        headers: { "content-type": "application/json" },
      }),
  });
  const result = await client.getSorafsManifest(manifestHex);
  assert.equal(result.manifest_id_hex, manifestHex);
  assert.equal(result.manifest_b64, manifestB64);
  assert.equal(result.chunk_profile_handle, "profile@v1");
});

test("getDaManifest fetches manifest bundle", async () => {
  let captured = null;
  const ticketHex = `0x${"ab".repeat(32)}`;
  const manifestB64 = Buffer.from("manifest-bytes").toString("base64");
  const chunkPlan = { chunks: [{ chunk_index: 0, digest: "ff".repeat(16) }] };
  const samplingPlan = {
    assignment_hash: "aa".repeat(32),
    sample_window: 4,
    samples: [{ index: 2, role: "global_parity", group: 1 }],
  };
  const manifestHashHex = "ff".repeat(32);
  const blockHashHex = "bb".repeat(32);
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: {
        storage_ticket: ticketHex.slice(2).toUpperCase(),
        client_blob_id: "cc".repeat(32),
        blob_hash: "dd".repeat(32),
        chunk_root: "ee".repeat(32),
        manifest_hash: manifestHashHex,
        lane_id: 7,
        epoch: 9,
        manifest_len: 123,
        manifest_norito: manifestB64,
        manifest: { version: 1 },
        chunk_plan: chunkPlan,
        sampling_plan: samplingPlan,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getDaManifest(ticketHex, {
    blockHashHex,
  });
  assert.equal(
    captured?.url,
    `${BASE_URL}/v1/da/manifests/${ticketHex.slice(2).toLowerCase()}?block_hash=${blockHashHex.toLowerCase()}`,
  );
  assert.equal(
    captured?.init?.headers?.get("accept"),
    "application/json",
  );
  assert.equal(result.storage_ticket_hex, ticketHex.slice(2).toLowerCase());
  assert.equal(result.manifest_b64, manifestB64);
  assert.equal(result.manifest_hash_hex, manifestHashHex.toLowerCase());
  assert.deepEqual(result.chunk_plan, chunkPlan);
  assert.deepEqual(result.sampling_plan?.assignment_hash_hex, samplingPlan.assignment_hash.toLowerCase());
  assert.equal(result.sampling_plan?.sample_window, samplingPlan.sample_window);
  assert.equal(result.sampling_plan?.samples[0].index, 2);
  assert(Buffer.isBuffer(result.manifest_bytes));
  assert.equal(
    result.manifest_bytes.toString("utf8"),
    Buffer.from("manifest-bytes").toString("utf8"),
  );
});

test("getDaManifestToDir writes manifest and plan artefacts", async () => {
  const ticketHex = `0x${"ab".repeat(32)}`;
  const manifestBytes = Buffer.from("manifest-bytes");
  const manifestB64 = manifestBytes.toString("base64");
  const chunkPlan = { chunks: [{ chunk_index: 0, digest: "ff".repeat(16) }] };
  const samplingPlan = {
    assignment_hash: "aa".repeat(32),
    sample_window: 2,
    samples: [{ index: 0, role: "data", group: 0 }],
  };
  const manifestHashHex = "ff".repeat(32);
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        storage_ticket: ticketHex.slice(2).toUpperCase(),
        client_blob_id: "cc".repeat(32),
        blob_hash: "dd".repeat(32),
        chunk_root: "ee".repeat(32),
        manifest_hash: manifestHashHex,
        lane_id: 7,
        epoch: 9,
        manifest_len: 123,
        manifest_norito: manifestB64,
        manifest: { version: 1 },
        chunk_plan: chunkPlan,
        sampling_plan: samplingPlan,
      },
      headers: { "content-type": "application/json" },
    });

  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const tmpDir = await fs.mkdtemp(path.join(process.cwd(), "tmp-js-da-manifest-"));
  try {
    const result = await client.getDaManifestToDir(ticketHex, { outputDir: tmpDir });
    const label = manifestHashHex.toLowerCase();
    const manifestPath = path.join(tmpDir, `manifest_${label}.norito`);
    const manifestJsonPath = path.join(tmpDir, `manifest_${label}.json`);
    const chunkPlanPath = path.join(tmpDir, `chunk_plan_${label}.json`);
    const samplingPlanPath = path.join(tmpDir, `sampling_plan_${label}.json`);

    assert.equal(result.paths.label, label);
    const persisted = await fs.readFile(manifestPath);
    assert.equal(persisted.toString(), manifestBytes.toString());
    const manifestJson = JSON.parse(await fs.readFile(manifestJsonPath, "utf8"));
    assert.equal(manifestJson.version, 1);
    const planJson = JSON.parse(await fs.readFile(chunkPlanPath, "utf8"));
    assert.equal(planJson.chunks[0].chunk_index, 0);
    const samplingJson = JSON.parse(await fs.readFile(samplingPlanPath, "utf8"));
    assert.equal(samplingJson.sample_window, samplingPlan.sample_window);
  } finally {
    await fs.rm(tmpDir, { recursive: true, force: true });
  }
});

test("fetchDaPayloadViaGateway fetches manifest bundle and invokes gateway", async (t) => {
  const ticketHex = `0x${"ab".repeat(32)}`;
  const blobHashHex = "dd".repeat(32);
  const manifestHashHex = "11".repeat(32);
  const planValue = [{ chunk_index: 0, offset: 0, length: 32, digest_blake3: "ff".repeat(32) }];
  const manifestB64 = Buffer.from("manifest").toString("base64");
  const chunkerHandle = "sorafs.sf1@1.0.0";
  const fetchImpl = async (url, _init) => {
    if (url.endsWith(`/v1/da/manifests/${ticketHex.slice(2)}`)) {
      return createResponse({
        status: 200,
        jsonData: {
          storage_ticket: ticketHex.slice(2),
          client_blob_id: "cc".repeat(32),
          blob_hash: blobHashHex,
          manifest_hash: manifestHashHex,
          chunk_root: "ee".repeat(32),
          lane_id: 1,
          epoch: 2,
          manifest_len: 42,
          manifest: { version: 1 },
          manifest_norito: manifestB64,
          chunk_plan: planValue,
        },
        headers: { "content-type": "application/json" },
      });
    }
    throw new Error(`unexpected request to ${url}`);
  };
  const gatewayResult = {
    manifestIdHex: blobHashHex,
    chunkerHandle: "sorafs.sf1@1.0.0",
    chunkCount: 1,
    assembledBytes: 512,
    payload: Buffer.from([1, 2, 3]),
    telemetryRegion: null,
    anonymity: {
      policy: "anon-guard-pq",
      status: "met",
      reason: "none",
      soranetSelected: 0,
      pqSelected: 0,
      classicalSelected: 0,
      classicalRatio: 0,
      pqRatio: 0,
      candidateRatio: 0,
      deficitRatio: 0,
      supplyDelta: 0,
      brownout: false,
      brownoutEffective: false,
      usesClassical: false,
    },
    providerReports: [],
    chunkReceipts: [],
    localProxyManifest: null,
    carVerification: null,
    metadata: {
      providerCount: 0,
      gatewayProviderCount: 1,
      providerMix: "gateway-only",
      transportPolicy: "soranet-first",
      transportPolicyOverride: false,
      transportPolicyOverrideLabel: null,
      anonymityPolicy: "anon-guard-pq",
      anonymityPolicyOverride: false,
      anonymityPolicyOverrideLabel: null,
      maxParallel: null,
      maxPeers: null,
      retryBudget: null,
      providerFailureThreshold: 1,
      assumeNowUnix: 0,
      telemetrySourceLabel: null,
      gatewayManifestProvided: false,
      gatewayManifestId: manifestHashHex,
      gatewayManifestCid: null,
      allowSingleSourceFallback: false,
      allowImplicitMetadata: false,
    },
  };
  const gatewayMock = t.mock.fn(() => gatewayResult);
  const client = new ToriiClient(BASE_URL, {
    fetchImpl,
    sorafsGatewayFetch: gatewayMock,
  });
  const providers = [
    {
      name: "alpha",
      providerIdHex: "bb".repeat(32),
      baseUrl: "https://gateway.test/",
      streamTokenB64: "dG9rZW4=",
    },
  ];
  const session = await client.fetchDaPayloadViaGateway({
    storageTicketHex: ticketHex,
    chunkerHandle,
    gatewayProviders: providers,
    fetchOptions: { allowSingleSourceFallback: true },
  });
  assert.equal(session.manifest.storage_ticket_hex, ticketHex.slice(2));
  assert.equal(session.manifestIdHex, manifestHashHex);
  assert.equal(session.chunkerHandle, chunkerHandle);
  assert.deepEqual(session.chunkPlan, planValue);
  assert.equal(session.gatewayResult, gatewayResult);
  assert.equal(gatewayMock.mock.callCount(), 1);
  const [manifestArg, handleArg, planJsonArg, providerArg] =
    gatewayMock.mock.calls[0].arguments;
  assert.equal(manifestArg, manifestHashHex);
  assert.equal(handleArg, chunkerHandle);
  assert.ok(planJsonArg.includes('"chunk_index":0'));
  assert.equal(providerArg.length, 1);
});

test("fetchDaPayloadViaGateway validates signal option", async () => {
  const manifestBundle = {
    storage_ticket_hex: "aa".repeat(32),
    client_blob_id_hex: "cc".repeat(32),
    blob_hash_hex: "bb".repeat(32),
    manifest_hash_hex: "99".repeat(32),
    chunk_root_hex: "dd".repeat(32),
    chunk_plan: [
      {
        chunk_index: 0,
        offset: 0,
        length: 1,
        digest_blake3: "ee".repeat(32),
      },
    ],
    manifest_bytes: Buffer.from("manifest-bytes"),
    manifest_len: 14,
    lane_id: 1,
    epoch: 1,
  };
  const gatewayProviders = [
    {
      name: "alpha",
      providerIdHex: "11".repeat(32),
      baseUrl: "https://gateway.test",
      streamTokenB64: bufferToBase64Url(Buffer.from("token")),
    },
  ];
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 200, jsonData: {} }),
    sorafsGatewayFetch: () => ({
      manifestIdHex: manifestBundle.manifest_hash_hex,
      chunkerHandle: "sorafs.sf1@1.0.0",
      chunkCount: 1,
      assembledBytes: 1,
      payload: Buffer.alloc(0),
      telemetryRegion: null,
      anonymity: null,
      providerReports: [],
      chunkReceipts: [],
      localProxyManifest: null,
      carVerification: null,
      metadata: {},
    }),
  });

  await assert.rejects(
    () =>
      client.fetchDaPayloadViaGateway({
        manifestBundle,
        chunkerHandle: "sorafs.sf1@1.0.0",
        gatewayProviders,
        fetchOptions: { allowSingleSourceFallback: true },
        signal: "not-a-signal",
      }),
    /fetchDaPayloadViaGateway options\.signal must be an AbortSignal/i,
  );
});

test("fetchDaPayloadViaGateway rejects non-boolean allowSingleSourceFallback", async () => {
  const manifestBundle = {
    storage_ticket_hex: "aa".repeat(32),
    client_blob_id_hex: "bb".repeat(32),
    blob_hash_hex: "cc".repeat(32),
    manifest_hash_hex: "cc".repeat(32),
    chunk_root_hex: "dd".repeat(32),
    lane_id: 1,
    epoch: 1,
    manifest_len: 64,
    manifest_b64: Buffer.from("manifest").toString("base64"),
    chunk_plan: [
      { chunk_index: 0, offset: 0, length: 32, digest_blake3: "ee".repeat(32) },
    ],
  };
  const stubBinding = {
    sorafsGatewayFetch: () => ({
      manifest_id_hex: manifestBundle.manifest_hash_hex,
      chunker_handle: "sorafs.sf1@1.0.0",
      chunk_count: 1,
      assembled_bytes: 1,
      payload: Buffer.alloc(0),
      telemetry_region: null,
      anonymity_policy: "anon-guard-pq",
      anonymity_status: "met",
      anonymity_reason: "none",
      anonymity_soranet_selected: 0,
      anonymity_pq_selected: 0,
      anonymity_classical_selected: 0,
      anonymity_classical_ratio: 0,
      anonymity_pq_ratio: 0,
      anonymity_candidate_ratio: 0,
      anonymity_deficit_ratio: 0,
      anonymity_supply_delta: 0,
      anonymity_brownout: false,
      anonymity_brownout_effective: false,
      anonymity_uses_classical: false,
      provider_reports: [],
      chunk_receipts: [],
      metadata: {
        provider_count: 2,
        gateway_provider_count: 2,
        provider_mix: "gateway-only",
        transport_policy: "soranet-first",
        transport_policy_override: false,
        transport_policy_override_label: null,
        anonymity_policy: "anon-guard-pq",
        anonymity_policy_override: false,
        anonymity_policy_override_label: null,
        max_parallel: null,
        max_peers: null,
        retry_budget: null,
        provider_failure_threshold: 1,
        assume_now_unix: 0,
        telemetry_source_label: null,
        gateway_manifest_provided: false,
        gateway_manifest_id: manifestBundle.manifest_hash_hex,
        gateway_manifest_cid: null,
        allow_single_source_fallback: false,
        allow_implicit_metadata: false,
      },
    }),
  };
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 200, jsonData: {} }),
    sorafsGatewayFetch: (manifestIdHex, chunkerHandle, planJson, providers, options) =>
      sorafsGatewayFetch(manifestIdHex, chunkerHandle, planJson, providers, {
        ...options,
        __nativeBinding: stubBinding,
      }),
  });
  await assert.rejects(
    () =>
      client.fetchDaPayloadViaGateway({
        manifestBundle,
        chunkerHandle: "sorafs.sf1@1.0.0",
        gatewayProviders: [
          {
            name: "alpha",
            providerIdHex: "11".repeat(32),
            baseUrl: "https://gateway.one",
            streamTokenB64: "dG9rZW4=",
          },
          {
            name: "beta",
            providerIdHex: "22".repeat(32),
            baseUrl: "https://gateway.two",
            streamTokenB64: "dG9rZW4y",
          },
        ],
        fetchOptions: { allowSingleSourceFallback: "true" },
      }),
    /allowSingleSourceFallback must be a boolean/i,
  );
});

test("fetchDaPayloadViaGateway rejects invalid stream tokens", async () => {
  const manifestBundle = {
    storage_ticket_hex: "aa".repeat(32),
    client_blob_id_hex: "bb".repeat(32),
    blob_hash_hex: "cc".repeat(32),
    manifest_hash_hex: "dd".repeat(32),
    chunk_root_hex: "ee".repeat(32),
    chunk_plan: [
      { chunk_index: 0, offset: 0, length: 32, digest_blake3: "ff".repeat(32) },
    ],
  };
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 200, jsonData: {} }),
    sorafsGatewayFetch: () => {
      throw new Error("unexpected sorafsGatewayFetch call");
    },
  });

  await assert.rejects(
    () =>
      client.fetchDaPayloadViaGateway({
        manifestBundle,
        chunkerHandle: "sorafs.sf1@1.0.0",
        gatewayProviders: [
          {
            name: "alpha",
            providerIdHex: "11".repeat(32),
            baseUrl: "https://gateway.one",
            streamTokenB64: "not-base64!!",
          },
        ],
        fetchOptions: { allowSingleSourceFallback: true },
      }),
    /streamTokenB64/,
  );
});

test("fetchDaPayloadViaGateway uses custom hooks", async (t) => {
  const manifestBytes = Buffer.from("sample-manifest");
  const manifestBundle = {
    storage_ticket_hex: "aa".repeat(32),
    blob_hash_hex: "bb".repeat(32),
    manifest_hash_hex: "bb".repeat(32),
    client_blob_id_hex: "cc".repeat(32),
    chunk_root_hex: "dd".repeat(32),
    chunk_plan: [{ chunk_index: 0, size: 1, offset: 0 }],
    manifest_bytes: manifestBytes,
    manifest_len: manifestBytes.length,
    lane_id: 1,
    epoch: 2,
  };
  const providers = [
    {
      name: "alpha",
      providerIdHex: `0x${"ee".repeat(32)}`,
      baseUrl: "https://gateway.test",
      streamTokenB64: bufferToBase64Url(Buffer.from("token")),
    },
  ];
  const gatewayMock = t.mock.fn(() => ({
    payload: Buffer.from("payload-bytes"),
  }));
  const summaryMock = t.mock.fn(() => ({ summary: "ok" }));
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 200, jsonData: {} }),
    sorafsGatewayFetch: gatewayMock,
    generateDaProofSummary: summaryMock,
  });
  const session = await client.fetchDaPayloadViaGateway({
    manifestBundle,
    chunkerHandle: "sorafs.sf1@1.0.0",
    gatewayProviders: providers,
    proofSummary: true,
  });
  assert.equal(session.manifest.blob_hash_hex, manifestBundle.blob_hash_hex);
  assert.equal(gatewayMock.mock.calls[0].arguments[0], manifestBundle.manifest_hash_hex);
  assert.equal(typeof session.proofSummary, "object");
  assert.equal(summaryMock.mock.callCount(), 1);
  const [manifestArg, payloadArg, optionsArg] = summaryMock.mock.calls[0].arguments;
  assert.ok(Buffer.isBuffer(manifestArg));
  assert.ok(Buffer.isBuffer(payloadArg));
  assert.deepEqual(optionsArg, {});
});

test("fetchDaPayloadViaGateway reuses provided manifest bundle", async (t) => {
  const gatewayResult = {
    manifestIdHex: "aa".repeat(32),
    chunkerHandle: "sorafs.sf2@2.0.0",
    chunkCount: 2,
    assembledBytes: 256,
    payload: Buffer.from([9]),
    telemetryRegion: "ci",
    anonymity: {
      policy: "anon-guard-pq",
      status: "met",
      reason: "none",
      soranetSelected: 0,
      pqSelected: 0,
      classicalSelected: 0,
      classicalRatio: 0,
      pqRatio: 0,
      candidateRatio: 0,
      deficitRatio: 0,
      supplyDelta: 0,
      brownout: false,
      brownoutEffective: false,
      usesClassical: false,
    },
    providerReports: [],
    chunkReceipts: [],
    localProxyManifest: null,
    carVerification: null,
    metadata: {
      providerCount: 0,
      gatewayProviderCount: 1,
      providerMix: "gateway-only",
      transportPolicy: "soranet-first",
      transportPolicyOverride: false,
      transportPolicyOverrideLabel: null,
      anonymityPolicy: "anon-guard-pq",
      anonymityPolicyOverride: false,
      anonymityPolicyOverrideLabel: null,
      maxParallel: null,
      maxPeers: null,
      retryBudget: null,
      providerFailureThreshold: 1,
      assumeNowUnix: 0,
      telemetrySourceLabel: null,
      gatewayManifestProvided: false,
      gatewayManifestId: "aa".repeat(32),
      gatewayManifestCid: null,
      allowSingleSourceFallback: false,
      allowImplicitMetadata: false,
    },
  };
  const gatewayMock = t.mock.fn(() => gatewayResult);
  const manifestBundle = {
    storage_ticket_hex: "ff".repeat(32),
    client_blob_id_hex: "11".repeat(32),
    blob_hash_hex: "aa".repeat(32),
    manifest_hash_hex: "aa".repeat(32),
    chunk_root_hex: "cc".repeat(32),
    lane_id: 1,
    epoch: 1,
    manifest_len: 64,
    manifest_b64: Buffer.from("manifest").toString("base64"),
    manifest_bytes: Buffer.from("manifest"),
    manifest_json: {
      chunking: { namespace: "sorafs", name: "sf2", semver: "2.0.0" },
    },
    chunk_plan: [{ chunk_index: 0, offset: 0, length: 32, digest_blake3: "ff".repeat(32) }],
  };
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("manifest fetch should not be called");
    },
    sorafsGatewayFetch: gatewayMock,
  });
  const session = await client.fetchDaPayloadViaGateway({
    manifestBundle,
    planJson: JSON.stringify(manifestBundle.chunk_plan),
    chunkerHandle: "sorafs.sf2@2.0.0",
    gatewayProviders: [
      {
        name: "beta",
        providerIdHex: "22".repeat(32),
        baseUrl: "https://gateway.example/",
        streamTokenB64: "c3R1Yg==",
      },
    ],
    fetchOptions: { allowSingleSourceFallback: true },
  });
  assert.equal(session.chunkerHandle, "sorafs.sf2@2.0.0");
  assert.equal(session.manifestIdHex, "aa".repeat(32));
  assert.equal(gatewayMock.mock.callCount(), 1);
  const [manifestArg] = gatewayMock.mock.calls[0].arguments;
  assert.equal(manifestArg, "aa".repeat(32));
});

test("fetchDaPayloadViaGateway accepts providers alias", async (t) => {
  const manifestBundle = {
    storage_ticket_hex: "12".repeat(32),
    client_blob_id_hex: "34".repeat(32),
    blob_hash_hex: "56".repeat(32),
    manifest_hash_hex: "65".repeat(32),
    chunk_root_hex: "78".repeat(32),
    lane_id: 7,
    epoch: 3,
    manifest_len: 64,
    manifest_b64: Buffer.from("manifest").toString("base64"),
    chunk_plan: [
      { chunk_index: 0, offset: 0, length: 32, digest_blake3: "aa".repeat(32) },
    ],
  };
  const providers = [
    {
      name: "gamma",
      providerIdHex: "98".repeat(32),
      baseUrl: "https://gateway.test",
      streamTokenB64: Buffer.from("token").toString("base64"),
    },
  ];
  const gatewayMock = t.mock.fn(() => ({
    manifestIdHex: manifestBundle.manifest_hash_hex,
    chunkerHandle: "sorafs.sf1@1.0.0",
    chunkCount: 1,
    assembledBytes: 1,
    payload: Buffer.alloc(0),
    telemetryRegion: null,
    anonymity: null,
    providerReports: [],
    chunkReceipts: [],
    localProxyManifest: null,
    carVerification: null,
    metadata: {},
  }));
  const client = new ToriiClient(BASE_URL, {
    sorafsGatewayFetch: gatewayMock,
  });
  const session = await client.fetchDaPayloadViaGateway({
    manifestBundle,
    chunkerHandle: "sorafs.sf1@1.0.0",
    chunkPlan: manifestBundle.chunk_plan,
    providers,
  });
  assert.equal(session.manifestIdHex, manifestBundle.manifest_hash_hex);
  assert.equal(gatewayMock.mock.callCount(), 1);
  const [, , , providerArg] = gatewayMock.mock.calls[0].arguments;
  const expectedProviders = providers.map((provider) => ({
    ...provider,
    privacyEventsUrl: null,
  }));
  assert.deepEqual(providerArg, expectedProviders);
});

test("fetchDaPayloadViaGateway attaches proof summary when requested", async (t) => {
  const ticketHex = `0x${"ab".repeat(32)}`;
  const blobHashHex = "ee".repeat(32);
  const manifestHashHex = "ff".repeat(32);
  const manifestB64 = Buffer.from("proof-manifest").toString("base64");
  const fetchImpl = async (url, _init) => {
    if (url.endsWith(`/v1/da/manifests/${ticketHex.slice(2)}`)) {
      return createResponse({
        status: 200,
        jsonData: {
        storage_ticket: ticketHex.slice(2),
        client_blob_id: "cc".repeat(32),
        blob_hash: blobHashHex,
        manifest_hash: manifestHashHex,
        chunk_root: "dd".repeat(32),
        lane_id: 1,
        epoch: 2,
        manifest_len: 42,
        manifest: { chunk_profile_handle: "sorafs.sf1@1.0.0" },
          manifest_norito: manifestB64,
          chunk_plan: [{ chunk_index: 0, offset: 0, length: 32, digest_blake3: "ff".repeat(32) }],
        },
        headers: { "content-type": "application/json" },
      });
    }
    throw new Error(`unexpected request to ${url}`);
  };
  const gatewayResult = {
    manifestIdHex: manifestHashHex,
    chunkerHandle: "sorafs.sf1@1.0.0",
    chunkCount: 1,
    assembledBytes: 128,
    payload: Buffer.from([5, 6, 7]),
    telemetryRegion: null,
    anonymity: {
      policy: "anon-guard-pq",
      status: "met",
      reason: "none",
      soranetSelected: 0,
      pqSelected: 0,
      classicalSelected: 0,
      classicalRatio: 0,
      pqRatio: 0,
      candidateRatio: 0,
      deficitRatio: 0,
      supplyDelta: 0,
      brownout: false,
      brownoutEffective: false,
      usesClassical: false,
    },
    providerReports: [],
    chunkReceipts: [],
    localProxyManifest: null,
    carVerification: null,
    metadata: {
      providerCount: 0,
      gatewayProviderCount: 1,
      providerMix: "gateway-only",
      transportPolicy: "soranet-first",
      transportPolicyOverride: false,
      transportPolicyOverrideLabel: null,
      anonymityPolicy: "anon-guard-pq",
      anonymityPolicyOverride: false,
      anonymityPolicyOverrideLabel: null,
      maxParallel: null,
      maxPeers: null,
      retryBudget: null,
      providerFailureThreshold: 1,
      assumeNowUnix: 0,
      telemetrySourceLabel: null,
      gatewayManifestProvided: false,
      gatewayManifestId: blobHashHex,
      gatewayManifestCid: null,
      allowSingleSourceFallback: false,
      allowImplicitMetadata: false,
    },
  };
  const gatewayMock = t.mock.fn(() => gatewayResult);
  const proofSummary = { blob_hash_hex: blobHashHex, proofs: [] };
  const proofMock = t.mock.fn(() => proofSummary);
  const client = new ToriiClient(BASE_URL, {
    fetchImpl,
    sorafsGatewayFetch: gatewayMock,
    generateDaProofSummary: proofMock,
  });
  const providers = [
    {
      name: "alpha",
      providerIdHex: "bb".repeat(32),
      baseUrl: "https://gateway.test/",
      streamTokenB64: "dG9rZW4=",
    },
  ];
  const session = await client.fetchDaPayloadViaGateway({
    storageTicketHex: ticketHex,
    chunkerHandle: "sorafs.sf1@1.0.0",
    gatewayProviders: providers,
    fetchOptions: { allowSingleSourceFallback: true },
    proofSummary: { sampleCount: 2, leafIndexes: [0] },
  });
  assert.equal(session.proofSummary, proofSummary);
  assert.equal(proofMock.mock.callCount(), 1);
  const [manifestArg, payloadArg, optionsArg] = proofMock.mock.calls[0].arguments;
  assert(Buffer.isBuffer(manifestArg));
  assert.equal(manifestArg.toString("base64"), manifestB64);
  assert.equal(payloadArg, gatewayResult.payload);
  assert.deepEqual(optionsArg, { sampleCount: 2, leafIndexes: [0] });
  assert.equal(gatewayMock.mock.callCount(), 1);
});

test("fetchDaPayloadViaGateway rejects invalid manifest_b64 for proof summary", async (t) => {
  const manifestBundle = {
    manifest_hash_hex: "aa".repeat(32),
    manifest_b64: "AAAA====",
    chunk_plan: [
      { chunk_index: 0, offset: 0, length: 1, digest_blake3: "ff".repeat(32) },
    ],
  };
  const gatewayMock = t.mock.fn(() => ({ payload: Buffer.from([1]) }));
  const client = new ToriiClient(BASE_URL, {
    sorafsGatewayFetch: gatewayMock,
  });
  await assert.rejects(
    () =>
      client.fetchDaPayloadViaGateway({
        manifestBundle,
        chunkerHandle: "sorafs.sf1@1.0.0",
        gatewayProviders: [
          {
            name: "alpha",
            providerIdHex: "bb".repeat(32),
            baseUrl: "https://gateway.test/",
            streamTokenB64: "dG9rZW4=",
          },
        ],
        proofSummary: true,
      }),
    /manifest_b64/,
  );
});

test("submitDaBlob rejects invalid pdp_commitment payloads", async () => {
  const digest = Array.from({ length: 32 }, (_, index) => index);
  const receipt = {
    client_blob_id: [digest],
    lane_id: 1,
    epoch: 2,
    blob_hash: [digest],
    chunk_root: [digest],
    manifest_hash: [digest],
    storage_ticket: [digest],
    pdp_commitment: "AAAA====",
    queued_at_unix: 1234,
    operator_signature: "aa".repeat(64),
    rent_quote: null,
  };
  const fetchImpl = async () =>
    createResponse({
      status: 202,
      jsonData: { status: "accepted", duplicate: false, receipt },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () =>
      client.submitDaBlob({
        payload: Buffer.from("car-bytes"),
        codec: "nexus_lane_sidecar",
        laneId: 11,
        epoch: 22,
        sequence: 33,
        privateKeyHex: "11".repeat(32),
        clientBlobId: Buffer.alloc(32, 0x11),
      }),
    /pdp_commitment/,
  );
});

nativeTest("submitDaBlob builds ingest payload and normalizes response", async () => {
  let captured = null;
  const digest = Array.from({ length: 32 }, (_, index) => index);
  const receipt = {
    client_blob_id: [digest],
    lane_id: 1,
    epoch: 2,
    blob_hash: [digest.map((value) => (value + 1) & 0xff)],
    chunk_root: [digest.map((value) => (value + 2) & 0xff)],
    manifest_hash: [digest.map((value) => (value + 3) & 0xff)],
    storage_ticket: [digest.map((value) => (value + 4) & 0xff)],
    pdp_commitment: Buffer.from("commitment").toString("base64"),
    queued_at_unix: 1234,
    operator_signature: "aa".repeat(64),
    rent_quote: {
      base_rent: 100,
      protocol_reserve: 25,
      provider_reward: 75,
      pdp_bonus: 5,
      potr_bonus: 3,
      egress_credit_per_gib: 2,
    },
  };
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 202,
      jsonData: { status: "accepted", duplicate: false, receipt },
      headers: {
        "content-type": "application/json",
        "sora-pdp-commitment": Buffer.from("header").toString("base64"),
      },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.submitDaBlob({
    payload: Buffer.from("car-bytes"),
    codec: "nexus_lane_sidecar",
    laneId: 11,
    epoch: 22,
    sequence: 33,
    retentionPolicy: { governanceTag: "da.demo", storageClass: "Warm" },
    metadata: [{ key: "content-type", value: "application/car" }],
    privateKeyHex: "11".repeat(32),
  });
  assert.equal(captured?.url, `${BASE_URL}/v1/da/ingest`);
  assert.equal(captured?.init?.method, "POST");
  assert.equal(
    captured?.init?.headers?.get("content-type"),
    "application/json",
  );
  assert.equal(captured?.init?.headers?.get("accept"), "application/json");
  const submitted = JSON.parse(captured?.init?.body ?? "{}");
  assert.equal(submitted.blob_class.class, "TaikaiSegment");
  assert.equal(submitted.codec[0], "nexus_lane_sidecar");
  assert.equal(submitted.metadata.items[0].key, "content-type");
  assert.equal(
    Buffer.from(submitted.metadata.items[0].value, "base64").toString("utf8"),
    "application/car",
  );
  const requestDigest = Buffer.from(submitted.client_blob_id[0]);
  assert.equal(
    requestDigest.toString("hex").toUpperCase(),
    result.artifacts.clientBlobIdHex,
  );
  assert.equal(result.status, "accepted");
  assert.equal(result.duplicate, false);
  assert.equal(result.pdpCommitmentHeader, Buffer.from("header").toString("base64"));
  assert.ok(result.receipt);
  assert.equal(result.receipt?.lane_id, 1);
  assert.equal(result.receipt?.blob_hash_bytes.length, 32);
  assert.equal(
    result.receipt?.client_blob_id_hex,
    Buffer.from(digest).toString("hex").toUpperCase(),
  );
  assert.deepEqual(result.receipt?.rent_quote, {
    base_rent_micro: "100",
    protocol_reserve_micro: "25",
    provider_reward_micro: "75",
    pdp_bonus_micro: "5",
    potr_bonus_micro: "3",
    egress_credit_per_gib_micro: "2",
  });
});

nativeTest("submitDaBlob writes artefacts when artifactDir is set", async () => {
  const digest = Array.from({ length: 32 }, (_, index) => index);
  const receipt = {
    client_blob_id: [digest],
    lane_id: 1,
    epoch: 2,
    blob_hash: [digest.map((value) => (value + 1) & 0xff)],
    chunk_root: [digest.map((value) => (value + 2) & 0xff)],
    manifest_hash: [digest.map((value) => (value + 3) & 0xff)],
    storage_ticket: [digest.map((value) => (value + 4) & 0xff)],
    pdp_commitment: Buffer.from("commitment").toString("base64"),
    queued_at_unix: 1234,
    operator_signature: "aa".repeat(64),
    rent_quote: null,
  };
  const fetchImpl = async () =>
    createResponse({
      status: 202,
      jsonData: { status: "accepted", duplicate: false, receipt },
      headers: {
        "content-type": "application/json",
        "sora-pdp-commitment": Buffer.from("header").toString("base64"),
      },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const dir = await fs.mkdtemp(path.join(process.cwd(), "tmp-js-da-submit-"));
  try {
    const result = await client.submitDaBlob({
      payload: Buffer.from("car-bytes"),
      codec: "nexus_lane_sidecar",
      laneId: 11,
      epoch: 22,
      sequence: 33,
      retentionPolicy: { governanceTag: "da.demo", storageClass: "Warm" },
      metadata: [{ key: "content-type", value: "application/car" }],
      privateKeyHex: "11".repeat(32),
      artifactDir: dir,
    });
    assert.ok(result.artifactPaths);
    const { artifactPaths } = result;
    const requestJson = JSON.parse(
      await fs.readFile(artifactPaths?.requestJsonPath ?? "", "utf8"),
    );
    assert.equal(requestJson.lane_id, 11);
    const receiptJson = JSON.parse(
      await fs.readFile(artifactPaths?.receiptJsonPath ?? "", "utf8"),
    );
    assert.equal(receiptJson?.queued_at_unix, receipt.queued_at_unix);
    const headersJson = JSON.parse(
      await fs.readFile(artifactPaths?.responseHeadersPath ?? "", "utf8"),
    );
    assert.equal(
      headersJson["sora-pdp-commitment"],
      Buffer.from("header").toString("base64"),
    );
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
});

nativeTest("submitDaBlob requires signing inputs", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 500 }) });
  await assert.rejects(
    () =>
      client.submitDaBlob({
        payload: Buffer.from("demo"),
        codec: "application/octet-stream",
      }),
    /privateKey/i,
  );
});

test("proveDaAvailabilityToDir persists CLI artefacts", async () => {
  const ticketHex = `0x${"ab".repeat(32)}`;
  const manifestBytes = Buffer.from("manifest");
  const manifestB64 = manifestBytes.toString("base64");
  const chunkPlan = [{ chunk_index: 0, offset: 0, length: 4, provider: "p1" }];
  const payload = Buffer.from("payload-bytes");
  const proofSummary = {
    blob_hash_hex: "11".repeat(32),
    chunk_root_hex: "22".repeat(32),
    por_root_hex: "33".repeat(32),
    leaf_count: 1,
    segment_count: 1,
    chunk_count: 1,
    sample_count: 1,
    sample_seed: 0,
    proof_count: 1,
    proofs: [
      {
        origin: "sampled",
        leaf_index: 0,
        chunk_index: 0,
        segment_index: 0,
        leaf_offset: 0,
        leaf_length: 1,
        segment_offset: 0,
        segment_length: 1,
        chunk_offset: 0,
        chunk_length: 1,
        payload_len: payload.length,
        chunk_digest_hex: "aa".repeat(32),
        chunk_root_hex: "bb".repeat(32),
        segment_digest_hex: "cc".repeat(32),
        leaf_digest_hex: "dd".repeat(32),
        leaf_bytes_b64: Buffer.from([0]).toString("base64"),
        segment_leaves_hex: [],
        chunk_segments_hex: [],
        chunk_roots_hex: [],
        verified: true,
      },
    ],
  };
  const fetchImpl = async (url) => {
    if (url.endsWith(`/v1/da/manifests/${ticketHex.slice(2).toLowerCase()}`)) {
      return createResponse({
        status: 200,
        jsonData: {
          storage_ticket: ticketHex.slice(2),
          client_blob_id: "cc".repeat(32),
          blob_hash: "dd".repeat(32),
          manifest_hash: "ff".repeat(32),
          chunk_root: "ee".repeat(32),
          lane_id: 7,
          epoch: 9,
          manifest_len: 7,
          manifest_norito: manifestB64,
          manifest: { version: 1 },
          chunk_plan: chunkPlan,
        },
        headers: { "content-type": "application/json" },
      });
    }
    return createResponse({ status: 404 });
  };
  const gatewayResult = {
    manifest_id_hex: "dd".repeat(32),
    chunker_handle: "sorafs.sf1@1.0.0",
    chunk_count: 1,
    assembled_bytes: payload.length,
    payload,
    telemetry_region: "region-x",
    provider_reports: [],
    chunk_receipts: [],
    metadata: {},
    scoreboard: [
      {
        provider_id: "ff".repeat(32),
        alias: "gw-alpha",
        raw_score: 10,
        normalized_weight: 1,
        eligibility: "eligible",
      },
    ],
  };
  const tmpDir = await fs.mkdtemp(path.join(process.cwd(), "tmp-js-da-prove-"));
  const client = new ToriiClient(BASE_URL, {
    fetchImpl,
    sorafsGatewayFetch: async () => gatewayResult,
    generateDaProofSummary: async () => proofSummary,
  });
  try {
    const result = await client.proveDaAvailabilityToDir({
      storageTicketHex: ticketHex,
      gatewayProviders: [
        {
          name: "alpha",
          providerIdHex: "bb".repeat(32),
          baseUrl: "https://gateway.test/",
          streamTokenB64: Buffer.from("token").toString("base64"),
        },
      ],
      fetchOptions: { allowSingleSourceFallback: true },
      proofSummary: { sampleCount: 1 },
      outputDir: tmpDir,
    });
    const label = ticketHex.slice(2).toLowerCase();
    const manifestPath = path.join(tmpDir, `manifest_${label}.norito`);
    const chunkPlanPath = path.join(tmpDir, `chunk_plan_${label}.json`);
    const payloadPath = path.join(tmpDir, `payload_${label}.car`);
    const proofPath = path.join(tmpDir, `proof_summary_${label}.json`);
    const scoreboardPath = path.join(tmpDir, "scoreboard.json");

    assert(result.gatewayResult.scoreboard);
    assert.ok(await fileExists(manifestPath));
    assert.ok(await fileExists(chunkPlanPath));
    assert.ok(await fileExists(payloadPath));
    assert.ok(await fileExists(proofPath));
    assert.ok(await fileExists(scoreboardPath));

    const scoreboardJson = JSON.parse(await fs.readFile(scoreboardPath, "utf8"));
    assert.equal(scoreboardJson[0].alias, "gw-alpha");
    const proofJson = JSON.parse(await fs.readFile(proofPath, "utf8"));
    assert.equal(proofJson.sample_count, 1);
    assert.equal(proofJson.manifest_path, manifestPath);
  } finally {
    await fs.rm(tmpDir, { recursive: true, force: true });
  }
});

test("submitSorafsUptimeObservation posts telemetry sample", async () => {
  let captured = null;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: { status: "recorded", uptime_secs: 540, observed_secs: 600 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.submitSorafsUptimeObservation({ uptimeSecs: 540, observedSecs: 600 });
  assert.equal(captured?.url, `${BASE_URL}/v1/sorafs/capacity/uptime`);
  assert.equal(captured?.init?.method, "POST");
  const parsed = JSON.parse(captured?.init?.body ?? "{}");
  assert.deepEqual(parsed, { uptime_secs: 540, observed_secs: 600 });
  assert.deepEqual(result, { status: "recorded", uptime_secs: 540, observed_secs: 600 });
});

test("submitSorafsUptimeObservation rejects unsupported input fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { status: "recorded", uptime_secs: 1, observed_secs: 1 },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () =>
      client.submitSorafsUptimeObservation({
        uptimeSecs: 1,
        observedSecs: 1,
        extra: true,
      }),
    /submitSorafsUptimeObservation input contains unsupported fields: extra/,
  );
});

test("recordSorafsPorChallenge normalizes base64 payloads", async () => {
  let captured = null;
  const challenge = Buffer.from("por-challenge");
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: { status: "accepted" },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.recordSorafsPorChallenge({ challenge });
  assert.equal(captured?.url, `${BASE_URL}/v1/sorafs/capacity/por-challenge`);
  const parsed = JSON.parse(captured?.init?.body ?? "{}");
  assert.equal(parsed.challenge_b64, challenge.toString("base64"));
  assert.deepEqual(result, { status: "accepted" });
});

test("recordSorafsPorChallenge rejects unsupported input fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { status: "accepted" },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () =>
      client.recordSorafsPorChallenge({
        challenge: Buffer.from("hello"),
        memo: "nope",
      }),
    /recordSorafsPorChallenge input contains unsupported fields: memo/,
  );
});

test("recordSorafsPorProof rejects unsupported input fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { status: "accepted" },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () =>
      client.recordSorafsPorProof({
        proof: Buffer.from("ok"),
        trailing: "field",
      }),
    /recordSorafsPorProof input contains unsupported fields: trailing/,
  );
});

test("recordSorafsPorVerdict rejects unsupported input fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { status: "accepted" },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () =>
      client.recordSorafsPorVerdict({
        verdict: Buffer.from("ok"),
        trailing: "field",
      }),
    /recordSorafsPorVerdict input contains unsupported fields: trailing/,
  );
});

test("submitSorafsPorObservation posts probe results", async () => {
  let captured = null;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: { status: "success", success: true },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.submitSorafsPorObservation({ success: true });
  assert.equal(captured?.url, `${BASE_URL}/v1/sorafs/capacity/por`);
  const parsed = JSON.parse(captured?.init?.body ?? "{}");
  assert.equal(parsed.success, true);
  assert.deepEqual(result, { status: "success", success: true });
});

test("submitSorafsPorObservation rejects unsupported input fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { status: "success", success: true },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () =>
      client.submitSorafsPorObservation({
        success: true,
        details: "invalid",
      }),
    /submitSorafsPorObservation input contains unsupported fields: details/,
  );
});

const invalidSorafsSignalCases = [
  {
    label: "submitSorafsUptimeObservation",
    invoke: (client) =>
      client.submitSorafsUptimeObservation({
        uptimeSecs: 1,
        observedSecs: 1,
        signal: "invalid",
      }),
    path: "submitSorafsUptimeObservation.options.signal",
  },
  {
    label: "recordSorafsPorChallenge",
    invoke: (client) =>
      client.recordSorafsPorChallenge({
        challenge: Buffer.from("challenge"),
        signal: "invalid",
      }),
    path: "recordSorafsPorChallenge.options.signal",
  },
  {
    label: "recordSorafsPorProof",
    invoke: (client) =>
      client.recordSorafsPorProof({
        proof: Buffer.from("proof"),
        signal: "invalid",
      }),
    path: "recordSorafsPorProof.options.signal",
  },
  {
    label: "recordSorafsPorVerdict",
    invoke: (client) =>
      client.recordSorafsPorVerdict({
        verdict: Buffer.from("verdict"),
        signal: "invalid",
      }),
    path: "recordSorafsPorVerdict.options.signal",
  },
  {
    label: "submitSorafsPorObservation",
    invoke: (client) =>
      client.submitSorafsPorObservation({
        success: true,
        signal: "invalid",
      }),
    path: "submitSorafsPorObservation.options.signal",
  },
];

for (const { label, invoke, path } of invalidSorafsSignalCases) {
  test(`${label} rejects non-AbortSignal \`signal\` option`, async () => {
    const client = new ToriiClient(BASE_URL, {
      fetchImpl: async () => {
        throw new Error("fetch should not run for invalid signal options");
      },
    });
    await assert.rejects(
      () => invoke(client),
      (error) => {
        assert(error instanceof ValidationError);
        assert.equal(error.code, ValidationErrorCode.INVALID_OBJECT);
        assert.equal(error.path, path);
        assert.match(
          error.message,
          new RegExp(`${label} options\\.signal must be an AbortSignal`),
        );
        return true;
      },
    );
  });
}

test("getSorafsPorStatus returns Norito bytes", async () => {
  const responseBytes = Buffer.from([1, 2, 3, 4]);
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      arrayData: responseBytes,
      headers: { "content-type": "application/x-norito" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const buffer = await client.getSorafsPorStatus({ providerHex: "f".repeat(64) });
  assert(buffer.equals(responseBytes));
});

test("SoraFS registry helpers reject non-object options", async () => {
  const client = new ToriiClient(BASE_URL);
  await assert.rejects(
    () => client.listSorafsAliases("invalid"),
    /listSorafsAliases options must be an object/,
  );
  await assert.rejects(
    () => client.listSorafsPinManifests("invalid"),
    /listSorafsPinManifests options must be an object/,
  );
  await assert.rejects(
    () => client.listSorafsReplicationOrders("invalid"),
    /listSorafsReplicationOrders options must be an object/,
  );
  await assert.rejects(
    () => client.getSorafsPinManifest("deadbeef", "invalid"),
    /getSorafsPinManifest options must be an object/,
  );
});

test("SoraFS registry helpers reject unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run for option validation");
    },
  });
  const cases = [
    [
      "listSorafsAliases",
      () => client.listSorafsAliases({ namespace: "sorafs", extra: true }),
      "extra",
    ],
    [
      "listSorafsPinManifests",
      () => client.listSorafsPinManifests({ status: "pending", bogus: true }),
      "bogus",
    ],
    [
      "listSorafsReplicationOrders",
      () =>
        client.listSorafsReplicationOrders({
          status: "pending",
          manifestDigestHex: "a".repeat(64),
          stray: 1,
        }),
      "stray",
    ],
  ];
  for (const [label, invoke, field] of cases) {
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(invoke, (error) => {
      assert(error instanceof TypeError);
      assert.match(
        error.message,
        new RegExp(`${label} options contains unsupported fields: ${field}`),
      );
      return true;
    });
  }
});

test("SoraFS POR helpers reject non-object options", async () => {
  const client = new ToriiClient(BASE_URL);
  await assert.rejects(
    () => client.getSorafsPorStatus("invalid"),
    /getSorafsPorStatus options must be an object/,
  );
  await assert.rejects(
    () => client.exportSorafsPorStatus("invalid"),
    /exportSorafsPorStatus options must be an object/,
  );
  await assert.rejects(
    () => client.getSorafsPorWeeklyReport("2026-W05", "invalid"),
    /getSorafsPorWeeklyReport options must be an object/,
  );
});

test("SoraFS POR helpers reject unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run for option validation");
    },
  });
  const cases = [
    [
      "getSorafsPorStatus",
      () => client.getSorafsPorStatus({ extra: true }),
      "extra",
    ],
    [
      "exportSorafsPorStatus",
      () => client.exportSorafsPorStatus({ extra: true }),
      "extra",
    ],
    [
      "getSorafsPorWeeklyReport",
      () => client.getSorafsPorWeeklyReport("2026-W05", { extra: true }),
      "extra",
    ],
  ];
  for (const [label, invoke, field] of cases) {
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(invoke, (error) => {
      assert(error instanceof TypeError);
      assert.match(
        error.message,
        new RegExp(`${label} options contains unsupported fields: ${field}`),
      );
      return true;
    });
  }
});

test("getSorafsPorWeeklyReport validates ISO week input", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run for invalid ISO week inputs");
    },
  });
  await assert.rejects(
    () => client.getSorafsPorWeeklyReport(""),
    (error) => expectValidationErrorFixture(error, "getSorafsPorWeeklyReport_iso_week_empty"),
  );
  await assert.rejects(
    () => client.getSorafsPorWeeklyReport("2026W05"),
    (error) => expectValidationErrorFixture(error, "getSorafsPorWeeklyReport_iso_week_format"),
  );
  await assert.rejects(
    // @ts-expect-error intentional invalid input
    () => client.getSorafsPorWeeklyReport(42),
    (error) => expectValidationErrorFixture(error, "getSorafsPorWeeklyReport_iso_week_type"),
  );
  await assert.rejects(
    () => client.getSorafsPorWeeklyReport({ year: 2026, week: 60 }),
    (error) => expectValidationErrorFixture(error, "getSorafsPorWeeklyReport_iso_week_range"),
  );
});

test("getSorafsPorWeeklyReport accepts ISO week objects", async () => {
  const payload = Buffer.from([1, 2, 3]);
  let requestedUrl = null;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url) => {
      requestedUrl = url;
      return createResponse({ status: 200, arrayData: payload });
    },
  });
  const result = await client.getSorafsPorWeeklyReport({ year: 2027, week: 5 });
  assert.equal(requestedUrl, `${BASE_URL}/v1/sorafs/por/report/2027-W05`);
  assert.ok(Buffer.isBuffer(result));
  assert.deepEqual(result, Buffer.from(payload));
});

test("SoraFS storage helpers reject non-object options", async () => {
  const client = new ToriiClient(BASE_URL);
  await assert.rejects(
    () => client.getSorafsStorageState("invalid"),
    /getSorafsStorageState options must be an object/,
  );
  await assert.rejects(
    () => client.getSorafsManifest("ab".repeat(32), "invalid"),
    /getSorafsManifest options must be an object/,
  );
  await assert.rejects(
    () => client.getDaManifest("ff".repeat(32), "invalid"),
    /getDaManifest options must be an object/,
  );
});

test("SoraFS storage helpers reject unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL);
  const cases = [
    ["getSorafsStorageState", () => client.getSorafsStorageState({ extra: true }), "extra"],
    [
      "getSorafsManifest",
      () => client.getSorafsManifest("ab".repeat(32), { unexpected: "nope" }),
      "unexpected",
    ],
    ["getDaManifest", () => client.getDaManifest("ff".repeat(32), { note: "skip" }), "note"],
  ];
  for (const [label, invoke, field] of cases) {
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(invoke, (error) => {
      assert(error instanceof TypeError);
      assert.match(
        error.message,
        new RegExp(`${label} options contains unsupported fields: ${field}`),
      );
      return true;
    });
  }
});

test("governance helpers validate options", async () => {
  const client = new ToriiClient(BASE_URL);
  const finalizePayload = {
    referendum_id: "ref-123",
    proposal_id: "ab".repeat(32),
  };
  const enactPayload = { proposal_id: "cd".repeat(32) };
  const councilPayload = {
    committee_size: 1,
    candidates: [
      {
        account_id: SAMPLE_ACCOUNT_FORMS.i105,
        variant: "Normal",
        pk_b64: Buffer.from("pk").toString("base64"),
        proof_b64: Buffer.from("proof").toString("base64"),
      },
    ],
  };

  await assert.rejects(
    () => client.getGovernanceCouncilCurrent("invalid"),
    /getGovernanceCouncilCurrent options must be an object/,
  );
  await assert.rejects(
    () => client.getGovernanceCouncilAudit("invalid"),
    /getGovernanceCouncilAudit options must be an object/,
  );
  await assert.rejects(
    () => client.getGovernanceCouncilAudit({ epoch: 1, extra: true }),
    /getGovernanceCouncilAudit options contains unsupported fields: extra/,
  );
  await assert.rejects(
    () => client.governanceFinalizeReferendum(finalizePayload, { signal: "nope" }),
    /governanceFinalizeReferendum options.signal must be an AbortSignal/,
  );
  await assert.rejects(
    () => client.governanceFinalizeReferendum(finalizePayload, { extra: true }),
    /governanceFinalizeReferendum options contains unsupported fields: extra/,
  );
  await assert.rejects(
    () => client.governanceEnactProposal(enactPayload, { signal: "nope" }),
    /governanceEnactProposal options.signal must be an AbortSignal/,
  );
  await assert.rejects(
    () => client.governanceEnactProposal(enactPayload, { extra: true }),
    /governanceEnactProposal options contains unsupported fields: extra/,
  );
  const optionTypeCases = [
    [
      "getGovernanceProposal",
      () => client.getGovernanceProposal("proposal-1", "invalid"),
      /getGovernanceProposal options must be an object/,
    ],
    [
      "getGovernanceReferendum",
      () => client.getGovernanceReferendum("ref-1", "invalid"),
      /getGovernanceReferendum options must be an object/,
    ],
    [
      "getGovernanceTally",
      () => client.getGovernanceTally("ref-1", "invalid"),
      /getGovernanceTally options must be an object/,
    ],
    [
      "governanceDeriveCouncilVrf",
      () => client.governanceDeriveCouncilVrf(councilPayload, "invalid"),
      /governanceDeriveCouncilVrf options must be an object/,
    ],
    [
      "governancePersistCouncil",
      () => client.governancePersistCouncil(councilPayload, "invalid"),
      /governancePersistCouncil options must be an object/,
    ],
  ];
  for (const [_label, invoke, error] of optionTypeCases) {
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(invoke, error);
  }
  const invalidSignalCases = [
    [
      "getGovernanceProposal",
      () => client.getGovernanceProposal("proposal-1", { signal: "nope" }),
      /getGovernanceProposal options.signal must be an AbortSignal/,
    ],
    [
      "getGovernanceReferendum",
      () => client.getGovernanceReferendum("ref-1", { signal: "nope" }),
      /getGovernanceReferendum options.signal must be an AbortSignal/,
    ],
    [
      "getGovernanceTally",
      () => client.getGovernanceTally("ref-1", { signal: "nope" }),
      /getGovernanceTally options.signal must be an AbortSignal/,
    ],
    [
      "governanceDeriveCouncilVrf",
      () => client.governanceDeriveCouncilVrf(councilPayload, { signal: "nope" }),
      /governanceDeriveCouncilVrf options.signal must be an AbortSignal/,
    ],
    [
      "governancePersistCouncil",
      () => client.governancePersistCouncil(councilPayload, { signal: "nope" }),
      /governancePersistCouncil options.signal must be an AbortSignal/,
    ],
  ];
  for (const [_label, invoke, error] of invalidSignalCases) {
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(invoke, error);
  }
  const extraFieldCases = [
    [
      "getGovernanceProposal",
      () => client.getGovernanceProposal("proposal-1", { extra: true }),
      /getGovernanceProposal options contains unsupported fields: extra/,
    ],
    [
      "getGovernanceReferendum",
      () => client.getGovernanceReferendum("ref-1", { extra: true }),
      /getGovernanceReferendum options contains unsupported fields: extra/,
    ],
    [
      "getGovernanceTally",
      () => client.getGovernanceTally("ref-1", { extra: true }),
      /getGovernanceTally options contains unsupported fields: extra/,
    ],
    [
      "governanceDeriveCouncilVrf",
      () => client.governanceDeriveCouncilVrf(councilPayload, { extra: true }),
      /governanceDeriveCouncilVrf options contains unsupported fields: extra/,
    ],
    [
      "governancePersistCouncil",
      () => client.governancePersistCouncil(councilPayload, { extra: true }),
      /governancePersistCouncil options contains unsupported fields: extra/,
    ],
  ];
  for (const [_label, invoke, error] of extraFieldCases) {
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(invoke, error);
  }
});

test("governance id validation surfaces structured errors", async () => {
  const client = new ToriiClient(BASE_URL);
  const cases = [
    [
      "getGovernanceProposal",
      () => client.getGovernanceProposal("  "),
      "proposalId",
    ],
    [
      "getGovernanceReferendum",
      // @ts-expect-error intentionally invalid type for validation path
      () => client.getGovernanceReferendum(null),
      "referendumId",
    ],
    [
      "getGovernanceTally",
      () => client.getGovernanceTally(undefined),
      "referendumId",
    ],
  ];
  for (const [label, invoke, path] of cases) {
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(invoke, (error) => {
      assert.ok(error instanceof ValidationError, `${label} should surface ValidationError`);
      assert.equal(error.code, ValidationErrorCode.INVALID_STRING);
      assert.equal(error.path, path);
      return true;
    });
  }
});

test("DA and UAID helpers reject non-object options", async () => {
  const client = new ToriiClient(BASE_URL);
  await assert.rejects(
    () => client.submitDaBlob("invalid"),
    /submitDaBlob options must be an object/,
  );
  const uaid = `uaid:${"11".repeat(32)}`;
  await assert.rejects(
    () => client.getUaidPortfolio(uaid, "invalid"),
    /getUaidPortfolio options must be an object/,
  );
  await assert.rejects(
    () => client.getUaidBindings(uaid, "invalid"),
    /getUaidBindings options must be an object/,
  );
  await assert.rejects(
    () => client.getUaidManifests(uaid, "invalid"),
    /getUaidManifests options must be an object/,
  );
});

test("submitIsoPacs008 posts XML payload", async () => {
  let captured;
  const payload = createIsoSubmissionPayload({ message_id: "msg-1" });
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 202,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const xml = "<Document>hello</Document>";
  const response = await client.submitIsoPacs008(xml);
  assert.equal(captured.url, `${BASE_URL}/v1/iso20022/pacs008`);
  assert.equal(captured.init.method, "POST");
  assert.equal(captured.init.headers["Content-Type"], "application/xml");
  assert.equal(captured.init.headers.Accept, "application/json");
  assert.ok(Buffer.isBuffer(captured.init.body));
  assert.equal(captured.init.body.toString("utf8"), xml);
  assert.deepEqual(response, payload);
});

test("submitIsoPacs009 posts XML payload", async () => {
  let captured;
  const payload = createIsoSubmissionPayload({ message_id: "msg-2" });
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 202,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const xml = "<Document>funding</Document>";
  const response = await client.submitIsoPacs009(xml, {
    contentType: "application/xml",
  });
  assert.equal(captured.url, `${BASE_URL}/v1/iso20022/pacs009`);
  assert.equal(captured.init.method, "POST");
  assert.equal(captured.init.headers["Content-Type"], "application/xml");
  assert.equal(captured.init.headers.Accept, "application/json");
  assert.ok(Buffer.isBuffer(captured.init.body));
  assert.equal(captured.init.body.toString("utf8"), xml);
  assert.deepEqual(response, payload);
});

test("submitIsoMessage builds pacs.008 XML and posts with defaults", async () => {
  const calls = [];
  const payload = createIsoSubmissionPayload({ message_id: "built-iso" });
  const fetchImpl = async (url, init) => {
    calls.push({ url, init });
    return createResponse({
      status: 202,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const response = await client.submitIsoMessage({
    messageId: "built-iso",
    instructionId: "instr-iso",
    amount: { currency: "EUR", value: "5.00" },
    instigatingAgent: { bic: "DEUTDEFF" },
    instructedAgent: { bic: "COBADEFF" },
  });

  assert.equal(calls.length, 1);
  const [call] = calls;
  assert.equal(call.url, `${BASE_URL}/v1/iso20022/pacs008`);
  assert.equal(call.init.method, "POST");
  assert.equal(call.init.headers["Content-Type"], "application/pacs008+xml");
  assert.equal(call.init.headers.Accept, "application/json");
  const xml = call.init.body?.toString("utf8") ?? "";
  assert.match(xml, /<MsgId>built-iso<\/MsgId>/);
  assert.match(xml, /<InstrId>instr-iso<\/InstrId>/);
  assert.match(xml, /<IntrBkSttlmAmt Ccy="EUR">5\.00<\/IntrBkSttlmAmt>/);
  assert.deepEqual(response, payload);
});

test("submitIsoMessage rejects unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("submitIsoMessage should reject before fetching");
    },
  });
  await assert.rejects(
    () =>
      client.submitIsoMessage(
        {
          messageId: "built-iso",
          instructionId: "instr-iso",
          amount: { currency: "EUR", value: "5.00" },
          instigatingAgent: { bic: "DEUTDEFF" },
          instructedAgent: { bic: "COBADEFF" },
        },
        { kind: "pacs.008", wait: { maxAttempts: 1 }, extra: true },
      ),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(error.message, /submitIsoMessage options contains unsupported fields: extra/);
      return true;
    },
  );
});

test("submitIsoMessage rejects mismatched kind aliases", async () => {
  let fetched = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      fetched = true;
      throw new Error("submitIsoMessage should reject before fetching");
    },
  });
  await assert.rejects(
    () =>
      client.submitIsoMessage(
        {
          messageId: "iso-mismatch",
          instructionId: "instr-iso",
          amount: { currency: "EUR", value: "5.00" },
          instigatingAgent: { bic: "DEUTDEFF" },
          instructedAgent: { bic: "COBADEFF" },
        },
        { kind: "pacs.008", messageKind: "pacs.009" },
      ),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(
        error.message,
        /submitIsoMessage options\.kind and options\.messageKind must match/,
      );
      return true;
    },
  );
  assert.equal(fetched, false);
});

test("submitIsoMessage rejects unsupported ISO message kinds", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("submitIsoMessage should reject before fetching");
    },
  });
  const messageFields = {
    messageId: "iso-invalid-kind",
    instructionId: "instr-invalid-kind",
    amount: { currency: "EUR", value: "1.23" },
    instigatingAgent: { bic: "DEUTDEFF" },
    instructedAgent: { bic: "COBADEFF" },
  };
  await assert.rejects(
    () => client.submitIsoMessage(messageFields, { kind: "pacs.007" }),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_STRING);
      assert.equal(error.path, "submitIsoMessage.options.kind");
      assert.match(error.message, /pacs\.008' or 'pacs\.009/);
      return true;
    },
  );
  await assert.rejects(
    () => client.submitIsoMessage(messageFields, { messageKind: " pacs.010 " }),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_STRING);
      assert.equal(error.path, "submitIsoMessage.options.messageKind");
      assert.match(error.message, /pacs\.008' or 'pacs\.009/);
      return true;
    },
  );
});

test("submitIsoMessage supports pacs.009 wait flow and reuses signals", async () => {
  const calls = [];
  const controller = new AbortController();
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should be mocked");
    },
  });
  client._request = async (_method, url, init = {}) => {
    calls.push({ url, init });
    if (url === "/v1/iso20022/pacs009") {
      return createResponse({
        status: 202,
        jsonData: createIsoSubmissionPayload({ message_id: "flow-009" }),
        headers: { "content-type": "application/json" },
      });
    }
    if (url === "/v1/iso20022/status/flow-009") {
      return createResponse({
        status: 200,
        jsonData: createIsoStatusPayload({
          message_id: "flow-009",
          status: "Accepted",
          transaction_hash: "h-1",
        }),
        headers: { "content-type": "application/json" },
      });
    }
    throw new Error(`unexpected url ${url}`);
  };
  const status = await client.submitIsoMessage(
    {
      instructionId: "flow-009",
      amount: { currency: "USD", value: "12.5" },
      instigatingAgent: { bic: "BOFAUS3N" },
      instructedAgent: { bic: "DEUTDEFF" },
      creationDateTime: "2026-02-01T00:00:00Z",
    },
    {
      kind: "pacs.009",
      signal: controller.signal,
      retryProfile: "iso-flow",
      wait: { maxAttempts: 1, pollIntervalMs: 0 },
    },
  );

  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, "/v1/iso20022/pacs009");
  assert.equal(calls[0].init.signal, controller.signal);
  assert.equal(calls[0].init.retryProfile, "iso-flow");
  assert.equal(calls[0].init.headers["Content-Type"], "application/pacs009+xml");
  assert.equal(calls[1].url, "/v1/iso20022/status/flow-009");
  assert.equal(calls[1].init.signal, controller.signal);
  assert.equal(calls[1].init.retryProfile, "iso-flow");
  assert.equal(status.status, "Accepted");
  assert.equal(status.transaction_hash, "h-1");
});

test("submitIsoMessage resolves accepted status without transaction hash when requested", async () => {
  const calls = [];
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should be mocked");
    },
  });
  client._request = async (_method, url, init = {}) => {
    calls.push({ url, init });
    if (url === "/v1/iso20022/pacs008") {
      return createResponse({
        status: 202,
        jsonData: createIsoSubmissionPayload({ message_id: "accept-no-tx" }),
        headers: { "content-type": "application/json" },
      });
    }
    if (url === "/v1/iso20022/status/accept-no-tx") {
      return createResponse({
        status: 200,
        jsonData: createIsoStatusPayload({
          message_id: "accept-no-tx",
          status: "Accepted",
          transaction_hash: null,
        }),
        headers: { "content-type": "application/json" },
      });
    }
    throw new Error(`unexpected url ${url}`);
  };

  const status = await client.submitIsoMessage(
    {
      messageId: "accept-no-tx",
      instructionId: "instr-accept-no-tx",
      amount: { currency: "EUR", value: "1.00" },
      instigatingAgent: { bic: "DEUTDEFF" },
      instructedAgent: { bic: "COBADEFF" },
    },
    {
      kind: "pacs.008",
      wait: {
        maxAttempts: 1,
        pollIntervalMs: 0,
        resolveOnAcceptedWithoutTransaction: true,
      },
    },
  );

  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, "/v1/iso20022/pacs008");
  assert.equal(calls[1].url, "/v1/iso20022/status/accept-no-tx");
  assert.equal(status.status, "Accepted");
  assert.equal(status.transaction_hash, null);
});

test("submitIsoPacs008AndWait reuses signal and retryProfile for polling", async () => {
  const calls = [];
  const controller = new AbortController();
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should be mocked");
    },
  });
  client._request = async (_method, url, init = {}) => {
    calls.push({ url, init });
    if (url === "/v1/iso20022/pacs008") {
      return createResponse({
        status: 202,
        jsonData: createIsoSubmissionPayload({ message_id: "reuse-008" }),
        headers: { "content-type": "application/json" },
      });
    }
    if (url === "/v1/iso20022/status/reuse-008") {
      return createResponse({
        status: 200,
        jsonData: createIsoStatusPayload({
          message_id: "reuse-008",
          status: "Accepted",
          transaction_hash: "0xiso008",
        }),
        headers: { "content-type": "application/json" },
      });
    }
    throw new Error(`unexpected url ${url}`);
  };

  const status = await client.submitIsoPacs008AndWait("<Document/>", {
    signal: controller.signal,
    retryProfile: "iso-wait",
    wait: { maxAttempts: 1, pollIntervalMs: 0 },
  });

  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, "/v1/iso20022/pacs008");
  assert.equal(calls[0].init.signal, controller.signal);
  assert.equal(calls[0].init.retryProfile, "iso-wait");
  assert.equal(calls[1].url, "/v1/iso20022/status/reuse-008");
  assert.equal(calls[1].init.signal, controller.signal);
  assert.equal(calls[1].init.retryProfile, "iso-wait");
  assert.equal(status.status, "Accepted");
  assert.equal(status.transaction_hash, "0xiso008");
});

test("getIsoMessageStatus fetches status payload", async () => {
  let requestedUrl;
  const payload = createIsoStatusPayload({
    message_id: "msg-2",
    transaction_hash: "abc",
    detail: "accepted",
    updated_at_ms: 123,
  });
  const fetchImpl = async (url) => {
    requestedUrl = url;
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const status = await client.getIsoMessageStatus("msg-2");
  assert.equal(requestedUrl, `${BASE_URL}/v1/iso20022/status/msg-2`);
  assert.deepEqual(status, payload);
});

test("getIsoMessageStatus forwards retryProfile to _request", async () => {
  let capturedRetryProfile;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("unexpected fetch");
    },
  });
  client._request = async (method, url, init = {}) => {
    capturedRetryProfile = init.retryProfile ?? null;
    return createResponse({
      status: 200,
      jsonData: createIsoStatusPayload({
        message_id: "rp",
        status: "Committed",
        transaction_hash: "0xabc",
      }),
      headers: { "content-type": "application/json" },
    });
  };
  const status = await client.getIsoMessageStatus("rp", { retryProfile: "iso-status" });
  assert.equal(status?.transaction_hash, "0xabc");
  assert.equal(capturedRetryProfile, "iso-status");
});

test("getIsoMessageStatus rejects invalid retryProfile overrides", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not fetch");
    },
  });
  await assert.rejects(
    () =>
      client.getIsoMessageStatus("msg-2", {
        // @ts-expect-error runtime validation should reject incorrect type
        retryProfile: 123,
      }),
    (error) => expectValidationErrorFixture(error, "getIsoMessageStatus_retry_profile_type"),
  );
  await assert.rejects(
    () =>
      client.getIsoMessageStatus("msg-2", {
        retryProfile: "   ",
      }),
    (error) => expectValidationErrorFixture(error, "getIsoMessageStatus_retry_profile_empty"),
  );
});

test("waitForIsoMessageStatus polls until a transaction hash arrives", async () => {
  const responses = [
    createIsoStatusPayload({ message_id: "msg-2", status: "Pending", transaction_hash: null }),
    createIsoStatusPayload({ message_id: "msg-2", status: "Accepted", transaction_hash: null }),
    createIsoStatusPayload({
      message_id: "msg-2",
      status: "Accepted",
      transaction_hash: "HASH-1",
      detail: "settled",
    }),
  ];
  let calls = 0;
  const fetchImpl = async (url) => {
    assert.equal(url, `${BASE_URL}/v1/iso20022/status/msg-2`);
    const payload = responses[Math.min(calls, responses.length - 1)];
    calls += 1;
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const seen = [];
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.waitForIsoMessageStatus("msg-2", {
    pollIntervalMs: 0,
    maxAttempts: 5,
    onPoll: ({ attempt, status }) => seen.push({ attempt, status: status?.status ?? null }),
  });
  assert.equal(result.transaction_hash, "HASH-1");
  assert.equal(calls, 3);
  assert.deepEqual(
    seen.map((entry) => entry.status),
    ["Pending", "Accepted", "Accepted"],
  );
});

test("waitForIsoMessageStatus resolves when accepted state is considered terminal", async () => {
  let calls = 0;
  const fetchImpl = async () => {
    calls += 1;
    return createResponse({
      status: 200,
      jsonData: createIsoStatusPayload({
        message_id: "msg-accept",
        status: "Accepted",
        transaction_hash: null,
      }),
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.waitForIsoMessageStatus("msg-accept", {
    resolveOnAcceptedWithoutTransaction: true,
    pollIntervalMs: 0,
    maxAttempts: 2,
  });
  assert.equal(calls, 1);
  assert.equal(result.status, "Accepted");
  assert.equal(result.transaction_hash, null);
});

test("waitForIsoMessageStatus surfaces onPoll errors", async () => {
  let calls = 0;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should be mocked");
    },
  });
  client._request = async () => {
    calls += 1;
    return createResponse({
      status: 200,
      jsonData: createIsoStatusPayload({
        message_id: "msg-onpoll-fail",
        status: "Pending",
        transaction_hash: null,
      }),
      headers: { "content-type": "application/json" },
    });
  };

  await assert.rejects(
    () =>
      client.waitForIsoMessageStatus("msg-onpoll-fail", {
        pollIntervalMs: 0,
        maxAttempts: 2,
        onPoll: async ({ attempt }) => {
          if (attempt === 1) {
            throw new Error("onPoll failure propagates");
          }
        },
      }),
    (error) => {
      assert(error instanceof Error);
      assert.match(error.message, /onPoll failure propagates/);
      return true;
    },
  );
  assert.equal(calls, 1);
});

test("waitForIsoMessageStatus forwards retryProfile to status polls", async () => {
  let calls = 0;
  const retryProfiles = [];
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should be mocked");
    },
  });
  client._request = async (_method, _url, init = {}) => {
    retryProfiles.push(init.retryProfile ?? null);
    calls += 1;
    const payload =
      calls === 1
        ? createIsoStatusPayload({
            message_id: "msg-retry",
            status: "Pending",
            transaction_hash: null,
          })
        : createIsoStatusPayload({
            message_id: "msg-retry",
            status: "Committed",
            transaction_hash: "HASH-2",
          });
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const status = await client.waitForIsoMessageStatus("msg-retry", {
    pollIntervalMs: 0,
    maxAttempts: 2,
    retryProfile: "iso-wait",
  });
  assert.equal(status.status, "Committed");
  assert.deepEqual(retryProfiles, ["iso-wait", "iso-wait"]);
});

test("waitForIsoMessageStatus throws when no terminal status is observed", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: createIsoStatusPayload({
        message_id: "msg-pending",
        status: "Pending",
        transaction_hash: null,
      }),
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () =>
      client.waitForIsoMessageStatus("msg-pending", {
        pollIntervalMs: 0,
        maxAttempts: 2,
      }),
    (error) => {
      assert(error instanceof IsoMessageTimeoutError);
      assert.equal(error.messageId, "msg-pending");
      assert.equal(error.attempts, 2);
      assert.equal(error.lastStatus?.status, "Pending");
      return true;
    },
  );
});

test("waitForIsoMessageStatus rejects invalid AbortSignal option", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not fetch");
    },
  });
  await assert.rejects(
    () =>
      client.waitForIsoMessageStatus("msg-signal", {
        // @ts-expect-error runtime validation should reject incorrect signal
        signal: {},
      }),
    (error) => expectValidationErrorFixture(error, "waitForIsoMessageStatus_invalid_signal"),
  );
});

test("waitForIsoMessageStatus rejects invalid retryProfile overrides", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not fetch");
    },
  });
  await assert.rejects(
    () =>
      client.waitForIsoMessageStatus("msg-retry", {
        // @ts-expect-error runtime validation should reject incorrect type
        retryProfile: 123,
      }),
    (error) => expectValidationErrorFixture(error, "waitForIsoMessageStatus_retry_profile_type"),
  );
  await assert.rejects(
    () =>
      client.waitForIsoMessageStatus("msg-retry", {
        retryProfile: "",
      }),
    (error) => expectValidationErrorFixture(error, "waitForIsoMessageStatus_retry_profile_empty"),
  );
});

test("waitForIsoMessageStatus rejects non-boolean resolveOnAcceptedWithoutTransaction", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not fetch");
    },
  });
  await assert.rejects(
    () =>
      client.waitForIsoMessageStatus("msg-resolve-invalid", {
        // @ts-expect-error runtime validation should reject non-boolean flag
        resolveOnAcceptedWithoutTransaction: "true",
      }),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(
        error.message,
        /wait\.resolveOnAcceptedWithoutTransaction must be a boolean/,
      );
      return true;
    },
  );
});

test("waitForIsoMessageStatus accepts resolveOnAccepted alias", async () => {
  let calls = 0;
  const fetchImpl = async () => {
    calls += 1;
    return createResponse({
      status: 200,
      jsonData: createIsoStatusPayload({
        message_id: "msg-alias",
        status: "Accepted",
        transaction_hash: null,
      }),
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.waitForIsoMessageStatus("msg-alias", {
    resolveOnAccepted: true,
    pollIntervalMs: 0,
    maxAttempts: 2,
  });
  assert.equal(calls, 1);
  assert.equal(result.status, "Accepted");
  assert.equal(result.transaction_hash, null);
});

test("waitForIsoMessageStatus rejects mismatched resolveOnAccepted flags", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not fetch");
    },
  });
  await assert.rejects(
    () =>
      client.waitForIsoMessageStatus("msg-alias-conflict", {
        resolveOnAccepted: true,
        resolveOnAcceptedWithoutTransaction: false,
      }),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(
        error.message,
        /wait\.resolveOnAccepted and wait\.resolveOnAcceptedWithoutTransaction must match when both are provided/,
      );
      return true;
    },
  );
});

test("waitForIsoMessageStatus forwards AbortSignal to status fetches", async () => {
  const controller = new AbortController();
  const signals = [];
  const fetchImpl = async (url, init) => {
    signals.push(init?.signal ?? null);
    return createResponse({
      status: 200,
      jsonData: createIsoStatusPayload({
        message_id: "msg-forward",
        status: "Accepted",
        transaction_hash: "tx-1",
      }),
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const status = await client.waitForIsoMessageStatus("msg-forward", {
    signal: controller.signal,
    pollIntervalMs: 0,
    maxAttempts: 1,
  });
  assert.equal(status.transaction_hash, "tx-1");
  assert.deepEqual(signals, [controller.signal]);
});

test("submitIsoPacs008AndWait submits payload then waits for completion", async () => {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url, init });
    if (url === `${BASE_URL}/v1/iso20022/pacs008`) {
      return createResponse({
        status: 202,
        jsonData: { message_id: "msg-submit", status: "Accepted" },
        headers: { "content-type": "application/json" },
      });
    }
    if (url === `${BASE_URL}/v1/iso20022/status/msg-submit`) {
      return createResponse({
        status: 200,
        jsonData: {
          message_id: "msg-submit",
          status: "Accepted",
          transaction_hash: "hash-final",
        },
        headers: { "content-type": "application/json" },
      });
    }
    throw new Error(`Unexpected URL ${url}`);
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const status = await client.submitIsoPacs008AndWait("<xml/>", {
    wait: { pollIntervalMs: 0, maxAttempts: 1 },
  });
  assert.equal(status.transaction_hash, "hash-final");
  assert.equal(calls.length, 2);
});

test("submitIsoPacs008AndWait rejects non-object options", async () => {
  let fetchCalled = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      fetchCalled = true;
      return createResponse({
        status: 202,
        jsonData: { message_id: "msg-opt" },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await assert.rejects(
    () => client.submitIsoPacs008AndWait("<xml/>", 42),
    (error) => expectValidationErrorFixture(error, "submitIsoPacs008_options_invalid"),
  );
  assert.equal(fetchCalled, false);
});

test("submitIsoPacs008AndWait rejects non-object wait overrides", async () => {
  let fetchCalled = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      fetchCalled = true;
      return createResponse({
        status: 202,
        jsonData: { message_id: "msg-opt" },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await assert.rejects(
    () =>
      client.submitIsoPacs008AndWait("<xml/>", {
        // @ts-expect-error verifying runtime guards
        wait: "invalid",
      }),
    (error) => expectValidationErrorFixture(error, "submitIsoPacs008_wait_invalid"),
  );
  assert.equal(fetchCalled, false);
});

test("submitIsoPacs008AndWait rejects when submission omits message_id", async () => {
  const calls = [];
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url, init) => {
      calls.push({ url, init });
      if (url === `${BASE_URL}/v1/iso20022/pacs008`) {
        return createResponse({
          status: 202,
          jsonData: { status: "Accepted" },
          headers: { "content-type": "application/json" },
        });
      }
      throw new Error(`Unexpected URL ${url}`);
    },
  });
  await assert.rejects(
    () => client.submitIsoPacs008AndWait("<xml/>"),
    /ISO pacs008 submission did not return a message_id/,
  );
  assert.deepEqual(calls.map((call) => call.url), [`${BASE_URL}/v1/iso20022/pacs008`]);
});

test("submitIsoPacs009AndWait submits payload then waits for completion", async () => {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url, init });
    if (url === `${BASE_URL}/v1/iso20022/pacs009`) {
      return createResponse({
        status: 202,
        jsonData: { message_id: "msg-pacs009", status: "Accepted" },
        headers: { "content-type": "application/json" },
      });
    }
    if (url === `${BASE_URL}/v1/iso20022/status/msg-pacs009`) {
      return createResponse({
        status: 200,
        jsonData: {
          message_id: "msg-pacs009",
          status: "Accepted",
          transaction_hash: "hash-009",
          detail: "queued",
        },
        headers: { "content-type": "application/json" },
      });
    }
    throw new Error(`Unexpected URL ${url}`);
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const status = await client.submitIsoPacs009AndWait("<xml/>", {
    wait: { pollIntervalMs: 0, maxAttempts: 2 },
  });
  assert.equal(status.transaction_hash, "hash-009");
  assert.equal(status.status, "Accepted");
  assert.equal(calls.length, 2);
});

test("submitIsoPacs009AndWait rejects non-object wait overrides", async () => {
  let fetchCalled = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      fetchCalled = true;
      return createResponse({
        status: 202,
        jsonData: { message_id: "msg-wait" },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await assert.rejects(
    () =>
      client.submitIsoPacs009AndWait("<xml/>", {
        // @ts-expect-error runtime ensures wait payloads are objects
        wait: 5,
      }),
    (error) => expectValidationErrorFixture(error, "submitIsoPacs009_wait_invalid"),
  );
  assert.equal(fetchCalled, false);
});

test("submitIsoPacs009AndWait rejects when submission omits message_id", async () => {
  const calls = [];
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url, init) => {
      calls.push({ url, init });
      if (url === `${BASE_URL}/v1/iso20022/pacs009`) {
        return createResponse({
          status: 202,
          jsonData: { status: "Accepted" },
          headers: { "content-type": "application/json" },
        });
      }
      throw new Error(`Unexpected URL ${url}`);
    },
  });
  await assert.rejects(
    () => client.submitIsoPacs009AndWait("<xml/>"),
    /ISO pacs009 submission did not return a message_id/,
  );
  assert.deepEqual(calls.map((call) => call.url), [`${BASE_URL}/v1/iso20022/pacs009`]);
});

test("submitTransaction posts norito payload and decodes receipt response", async () => {
  const payload = new Uint8Array([0xde, 0xad]);
  const receiptJson = JSON.stringify({
    payload: {
      tx_hash: "aa".repeat(32),
      submitted_at_ms: 1,
      submitted_at_height: 2,
      signer: "ed0120" + "bb".repeat(32),
    },
    signature: "ed25519:" + "cc".repeat(64),
  });
  const fetchImpl = async (url, init) => {
    if (url === `${BASE_URL}/v1/node/capabilities`) {
      return createResponse({
        status: 200,
        jsonData: {
          abi_version: 1,
          data_model_version: 1,
          crypto: {
            sm: {
              enabled: false,
              default_hash: "sha2_256",
              allowed_signing: ["ed25519"],
              sm2_distid_default: "",
              openssl_preview: false,
              acceleration: {
                scalar: true,
                neon_sm3: false,
                neon_sm4: false,
                policy: "scalar-only",
              },
            },
            curves: {
              registry_version: 1,
              allowed_curve_ids: [1],
            },
          },
        },
        headers: { "content-type": "application/json" },
      });
    }
    assert.equal(url, `${BASE_URL}/v1/pipeline/transactions`);
    assert.equal(init.method, "POST");
    assert.equal(init.headers["Content-Type"], "application/x-norito");
    assert.equal(init.headers.Accept, "application/x-norito, application/json");
    assert.ok(Buffer.isBuffer(init.body));
    assert.deepEqual([...init.body.values()], [0xde, 0xad]);
    return createResponse({
      status: 202,
      arrayData: new Uint8Array([0x01, 0x02, 0x03]),
      headers: { "content-type": "application/x-norito" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const originalBinding = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    decodeTransactionReceiptJson: (buffer) => {
      assert.ok(Buffer.isBuffer(buffer));
      return receiptJson;
    },
  };
  try {
    const result = await client.submitTransaction(payload);
    assert.deepEqual(result, JSON.parse(receiptJson));
  } finally {
    if (originalBinding === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = originalBinding;
    }
  }
});

test("submitTransaction retries transient failures via pipeline profile", async () => {
  const payload = new Uint8Array([0xaa]);
  let attempts = 0;
  const fetchImpl = async (url, init) => {
    if (url === `${BASE_URL}/v1/node/capabilities`) {
      return createResponse({
        status: 200,
        jsonData: {
          abi_version: 1,
          data_model_version: 1,
          crypto: {
            sm: {
              enabled: false,
              default_hash: "sha2_256",
              allowed_signing: ["ed25519"],
              sm2_distid_default: "",
              openssl_preview: false,
              acceleration: {
                scalar: true,
                neon_sm3: false,
                neon_sm4: false,
                policy: "scalar-only",
              },
            },
            curves: {
              registry_version: 1,
              allowed_curve_ids: [1],
            },
          },
        },
        headers: { "content-type": "application/json" },
      });
    }
    attempts += 1;
    assert.equal(url, `${BASE_URL}/v1/pipeline/transactions`);
    assert.equal(init.method, "POST");
    if (attempts === 1) {
      return createResponse({ status: 503, jsonData: { error: "busy" } });
    }
    return createResponse({
      status: 202,
      jsonData: { ok: true },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl, maxRetries: 0 });
  const response = await client.submitTransaction(payload);
  assert.deepEqual(response, { ok: true });
  assert.equal(attempts, 2);
});

test("submitTransaction tolerates missing node capabilities advert", async () => {
  const payload = new Uint8Array([0xde, 0xad]);
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url, init });
    if (url === `${BASE_URL}/v1/node/capabilities`) {
      return createResponse({
        status: 404,
        jsonData: { error: "not found" },
        headers: { "content-type": "application/json" },
      });
    }
    assert.equal(url, `${BASE_URL}/v1/pipeline/transactions`);
    assert.equal(init.method, "POST");
    return createResponse({
      status: 202,
      jsonData: { ok: true },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const response = await client.submitTransaction(payload);
  assert.deepEqual(response, { ok: true });
  assert.deepEqual(calls.map((call) => call.url), [
    `${BASE_URL}/v1/node/capabilities`,
    `${BASE_URL}/v1/pipeline/transactions`,
  ]);
});

test("submitTransaction rejects mismatched data model version", async () => {
  const payload = new Uint8Array([0x01]);
  const fetchImpl = async (url) => {
    if (url === `${BASE_URL}/v1/node/capabilities`) {
      return createResponse({
        status: 200,
        jsonData: {
          abi_version: 1,
          data_model_version: 9,
          crypto: {
            sm: {
              enabled: false,
              default_hash: "sha2_256",
              allowed_signing: ["ed25519"],
              sm2_distid_default: "",
              openssl_preview: false,
              acceleration: {
                scalar: true,
                neon_sm3: false,
                neon_sm4: false,
                policy: "scalar-only",
              },
            },
            curves: {
              registry_version: 1,
              allowed_curve_ids: [1],
            },
          },
        },
        headers: { "content-type": "application/json" },
      });
    }
    throw new Error(`Unexpected URL ${url}`);
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.submitTransaction(payload),
    (error) => {
      assert(error instanceof ToriiDataModelCompatibilityError);
      assert.equal(error.expected, 1);
      assert.equal(error.actual, 9);
      return true;
    },
  );
});

test("getTransactionStatus queries pipeline endpoint", async () => {
  const txHash = "ab".repeat(32);
  const hashParam = "cd".repeat(32);
  const fetchImpl = async (url) => {
    assert.equal(
      url,
      `${BASE_URL}/v1/pipeline/transactions/status?hash=${hashParam}&scope=auto`,
    );
    return createResponse({
      status: 200,
      jsonData: {
        kind: "Transaction",
        content: { hash: txHash, status: { kind: "Committed", content: null } },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
    const result = await client.getTransactionStatus(hashParam);
    assert.deepEqual(result, {
      kind: "Transaction",
      content: { hash: txHash, status: { kind: "Committed", content: null } },
    });
  });

test("getTransactionStatus fans out to alternate endpoints in auto scope", async () => {
  const hashHex = "ef".repeat(32);
  const fallbackBaseUrl = "https://torii-sbp.soramitsu.io";
  const seenUrls = [];
  const fetchImpl = async (url) => {
    seenUrls.push(url);
    if (url === `${BASE_URL}/v1/pipeline/transactions/status?hash=${hashHex}&scope=auto`) {
      return createResponse({ status: 404 });
    }
    if (
      url ===
      `${fallbackBaseUrl}/v1/pipeline/transactions/status?hash=${hashHex}&scope=auto`
    ) {
      return createResponse({
        status: 200,
        jsonData: {
          kind: "Transaction",
          content: { hash: hashHex, status: { kind: "Committed", content: null } },
        },
        headers: { "content-type": "application/json" },
      });
    }
    throw new Error(`Unexpected URL ${url}`);
  };
  const client = new ToriiClient(BASE_URL, {
    fetchImpl,
    statusEndpoints: [fallbackBaseUrl],
  });
  const payload = await client.getTransactionStatus(hashHex);
  assert.equal(seenUrls.length, 2);
  assert.equal(payload?.resolved_from, fallbackBaseUrl);
});

test("getTransactionStatus local scope does not fan out", async () => {
  const hashHex = "01".repeat(32);
  const fallbackBaseUrl = "https://torii-sbp.soramitsu.io";
  const seenUrls = [];
  const fetchImpl = async (url) => {
    seenUrls.push(url);
    if (url === `${BASE_URL}/v1/pipeline/transactions/status?hash=${hashHex}&scope=local`) {
      return createResponse({ status: 404 });
    }
    throw new Error(`Unexpected URL ${url}`);
  };
  const client = new ToriiClient(BASE_URL, {
    fetchImpl,
    statusEndpoints: [fallbackBaseUrl],
  });
  const payload = await client.getTransactionStatus(hashHex, { scope: "local" });
  assert.equal(payload, null);
  assert.deepEqual(seenUrls, [
    `${BASE_URL}/v1/pipeline/transactions/status?hash=${hashHex}&scope=local`,
  ]);
});

test("getTransactionStatus forwards signal to fetch", async () => {
  const hashHex = "ab".repeat(32);
  const controller = new AbortController();
  let capturedSignal = null;
  const fetchImpl = async (_url, init = {}) => {
    capturedSignal = init.signal ?? null;
    return createResponse({ status: 404 });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.getTransactionStatus(hashHex, { signal: controller.signal });
  assert.equal(capturedSignal, controller.signal);
});

test("getTransactionStatus validates signal option type", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 404 }) });
  await assert.rejects(
    () =>
      client.getTransactionStatus("ab".repeat(32), {
        // @ts-expect-error runtime validation should reject incorrect signal
        signal: {},
      }),
    /getTransactionStatus options\.signal must be an AbortSignal/,
  );
});

test("getTransactionStatus rejects unsupported options", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 404 }) });
  await assert.rejects(
    () =>
      client.getTransactionStatus("ab".repeat(32), {
        extra: true,
      }),
    /getTransactionStatus options contains unsupported fields: extra/,
  );
});

test("getTransactionStatus validates allowShortHash option type", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 404 }) });
  await assert.rejects(
    () =>
      client.getTransactionStatus("ab".repeat(32), {
        // @ts-expect-error runtime validation should reject non-boolean allowShortHash
        allowShortHash: "yes",
      }),
    /getTransactionStatus options\.allowShortHash must be a boolean when provided/,
  );
});

test("getTransactionStatus validates scope option", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 404 }) });
  await assert.rejects(
    () =>
      client.getTransactionStatus("ab".repeat(32), {
        scope: "invalid",
      }),
    /getTransactionStatus options\.scope must be one of: local, auto, global/,
  );
});

  test("getTransactionStatus retries 425 via pipeline profile", async () => {
    const hash = "ab".repeat(32);
    let attempts = 0;
    const fetchImpl = async (url) => {
      attempts += 1;
      assert.equal(
        url,
        `${BASE_URL}/v1/pipeline/transactions/status?hash=${hash}&scope=auto`,
      );
      if (attempts === 1) {
        return createResponse({ status: 425, jsonData: { status: "TooEarly" } });
      }
      return createResponse({
        status: 200,
        jsonData: {
          kind: "Transaction",
          content: { hash, status: { kind: "Committed", content: null } },
        },
        headers: { "content-type": "application/json" },
      });
    };
    const client = new ToriiClient(BASE_URL, { fetchImpl, maxRetries: 0 });
    const result = await client.getTransactionStatus(hash);
    assert.equal(attempts, 2);
    assert.equal(result?.content?.status?.kind, "Committed");
  });

test("getTransactionStatus returns null when Torii responds without a body", async () => {
  const fetchImpl = async () => createResponse({ status: 202 });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getTransactionStatus("ef".repeat(32));
  assert.equal(result, null);
});

test("getTransactionStatus returns null when Torii responds with 404", async () => {
  const hashHex = "aa".repeat(32);
  const fetchImpl = async (url) => {
    assert.equal(
      url,
      `${BASE_URL}/v1/pipeline/transactions/status?hash=${hashHex}&scope=auto`,
    );
    return createResponse({ status: 404 });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getTransactionStatus(hashHex);
  assert.equal(result, null);
});

test("getTransactionStatus matches shared error-message contract fixture", async () => {
  const hashHex = "ab".repeat(32);
  for (const fixtureCase of txStatusErrorMessageContract.cases) {
    const headers = {};
    if (typeof fixtureCase.content_type === "string" && fixtureCase.content_type) {
      headers["content-type"] = fixtureCase.content_type;
    }
    if (
      typeof fixtureCase.reject_code_header === "string" &&
      fixtureCase.reject_code_header
    ) {
      const rejectHeaderName =
        typeof fixtureCase.reject_code_header_name === "string" &&
        fixtureCase.reject_code_header_name
          ? fixtureCase.reject_code_header_name
          : "x-iroha-reject-code";
      headers[rejectHeaderName] = fixtureCase.reject_code_header;
    }
    const fetchImpl = async () =>
      createResponse({
        status: fixtureCase.status_code,
        jsonData: fixtureCase.body_json ?? {},
        textBody: fixtureCase.body_text,
        headers,
      });
    const client = new ToriiClient(BASE_URL, { fetchImpl, maxRetries: 0 });
    await assert.rejects(
      () => client.getTransactionStatus(hashHex),
      (error) => {
        assert(error instanceof ToriiHttpError, `${fixtureCase.id}: expected ToriiHttpError`);
        assert.equal(error.status, fixtureCase.status_code, `${fixtureCase.id}: status mismatch`);
        if (fixtureCase.expected_reject_code) {
          assert.equal(
            error.rejectCode,
            fixtureCase.expected_reject_code,
            `${fixtureCase.id}: reject code mismatch`,
          );
        }
        if (fixtureCase.expected_message) {
          assert.equal(
            error.errorMessage,
            fixtureCase.expected_message,
            `${fixtureCase.id}: message mismatch`,
          );
        }
        if (fixtureCase.expected_message_length) {
          assert.equal(
            error.errorMessage?.length,
            fixtureCase.expected_message_length,
            `${fixtureCase.id}: message length mismatch`,
          );
        }
        if (fixtureCase.expected_message_suffix) {
          assert.equal(
            error.errorMessage?.endsWith(fixtureCase.expected_message_suffix),
            true,
            `${fixtureCase.id}: message suffix mismatch`,
          );
        }
        return true;
      },
      `${fixtureCase.id}: getTransactionStatus should reject`,
    );
  }
});

test("getTransactionStatus surfaces nested JSON error message and reject code", async () => {
  const hashHex = "ab".repeat(32);
  const fetchImpl = async () =>
    createResponse({
      status: 400,
      jsonData: {
        error: {
          detail: "missing build claim for transaction status",
        },
      },
      headers: {
        "content-type": "application/json",
        "x-iroha-reject-code": "build_claim_missing",
      },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getTransactionStatus(hashHex),
    (error) => {
      assert(error instanceof ToriiHttpError);
      assert.equal(error.status, 400);
      assert.equal(error.rejectCode, "build_claim_missing");
      assert.equal(error.code, "build_claim_missing");
      assert.equal(error.errorMessage, "missing build claim for transaction status");
      return true;
    },
  );
});

test("getTransactionStatus surfaces errors-array messages", async () => {
  const hashHex = "cd".repeat(32);
  const fetchImpl = async () =>
    createResponse({
      status: 422,
      jsonData: {
        errors: [{ message: "status query validation failed" }, { message: "hash malformed" }],
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getTransactionStatus(hashHex),
    (error) => {
      assert(error instanceof ToriiHttpError);
      assert.equal(error.status, 422);
      assert.equal(error.errorMessage, "status query validation failed");
      return true;
    },
  );
});

test("getTransactionStatus falls back to compact JSON for message-less errors", async () => {
  const hashHex = "ef".repeat(32);
  const fetchImpl = async () =>
    createResponse({
      status: 422,
      jsonData: { status: "invalid", code: "E123" },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl, maxRetries: 0 });
  await assert.rejects(
    () => client.getTransactionStatus(hashHex),
    (error) => {
      assert(error instanceof ToriiHttpError);
      assert.equal(error.status, 422);
      assert.equal(error.errorMessage, '{"code":"E123","status":"invalid"}');
      return true;
    },
  );
});

test("getTransactionStatus truncates oversized plain-text errors", async () => {
  const hashHex = "10".repeat(32);
  const oversized = "x".repeat(700);
  const fetchImpl = async () =>
    createResponse({
      status: 422,
      textBody: oversized,
      headers: { "content-type": "text/plain" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl, maxRetries: 0 });
  await assert.rejects(
    () => client.getTransactionStatus(hashHex),
    (error) => {
      assert(error instanceof ToriiHttpError);
      assert.equal(error.status, 422);
      assert.equal(error.errorMessage.length, 515);
      assert.equal(error.errorMessage.endsWith("..."), true);
      return true;
    },
  );
});

test("getTransactionStatus rejects invalid hashes", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200 }) });
  await assert.rejects(
    () => client.getTransactionStatus("abc123"),
    /getTransactionStatus\.hashHex/,
  );
});

test("getTransactionStatus rejects on malformed payloads", async () => {
  const hashHex = "12".repeat(32);
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: { kind: "Transaction", content: { status: { kind: "Approved" } } },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(client.getTransactionStatus(hashHex), /content\.hash/);
});

test("getTransactionStatusTyped normalises pipeline payload", async () => {
  const hashHex = "cd".repeat(32);
  const payload = {
    kind: "Transaction",
    content: {
      hash: hashHex,
      authority: FIXTURE_ALICE_ID,
      status: { kind: "Committed", content: { receipt: "ok" } },
    },
  };
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getTransactionStatusTyped(hashHex);
  assert.ok(result, "typed payload should be returned");
  assert.equal(result?.kind, "Transaction");
  assert.equal(result?.hashHex, hashHex);
  assert.equal(result?.authority, FIXTURE_ALICE_ID);
  assert.equal(result?.status?.kind, "Committed");
  assert.deepEqual(result?.status?.content, { receipt: "ok" });
  assert.deepEqual(result?.raw.kind, "Transaction");
});

test("getTransactionStatusTyped returns null for empty payload", async () => {
  const hashHex = "ef".repeat(32);
  const fetchImpl = async () =>
    createResponse({
      status: 204,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getTransactionStatusTyped(hashHex);
  assert.equal(result, null);
});

test("getTransactionStatusTyped rejects on malformed payload", async () => {
  const hashHex = "34".repeat(32);
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: "not-an-object",
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(() => client.getTransactionStatusTyped(hashHex), /must be an object/);
});

test("getPipelineRecovery fetches the recovery sidecar", async () => {
  const fixture = createPipelineRecoveryPayload({ height: 7 });
  let capturedUrl;
  const fetchImpl = async (url, init) => {
    capturedUrl = url;
    assert.equal(init?.method, "GET");
    return createResponse({
      status: 200,
      jsonData: fixture,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.getPipelineRecovery(7n);
  assert.equal(capturedUrl, `${BASE_URL}/v1/pipeline/recovery/7`);
  assert.deepEqual(payload, fixture);
});

test("getPipelineRecovery returns null for missing heights", async () => {
  const fetchImpl = async () => createResponse({ status: 404 });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.getPipelineRecovery(99);
  assert.equal(payload, null);
});

test("getPipelineRecovery throws when Torii omits JSON", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: null,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getPipelineRecovery(1),
    /pipeline recovery endpoint returned no payload/,
  );
});

test("getPipelineRecoveryTyped normalises dag + transaction snapshots", async () => {
  const payload = createPipelineRecoveryPayload({
    dag: { fingerprint: fakeHashHex(0x10), key_count: 9 },
    txs: [
      { hash: fakeHashHex(0x33), reads: ["  state::foo  "], writes: ["World.bar"] },
      { hash: fakeHashHex(0x44), reads: [], writes: ["ledger.accounts"] },
    ],
    height: 123,
  });
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getPipelineRecoveryTyped(123);
  assert.deepEqual(result, {
    format: "dag-json",
    height: 123,
    dag: {
      fingerprintHex: payload.dag.fingerprint.toLowerCase(),
      keyCount: 9,
    },
    txs: [
      {
        hashHex: payload.txs[0].hash.toLowerCase(),
        reads: ["state::foo"],
        writes: ["World.bar"],
      },
      {
        hashHex: payload.txs[1].hash.toLowerCase(),
        reads: [],
        writes: ["ledger.accounts"],
      },
    ],
  });
});

test("getPipelineRecoveryTyped rejects malformed payloads", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: { format: "", height: 1, dag: {}, txs: {} },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getPipelineRecoveryTyped(1),
    /pipeline recovery response\.format/,
  );
});

test("extractPipelineStatusKind returns nested status kind", () => {
  const payload = {
    kind: "Transaction",
    content: { status: { kind: "Committed" } },
  };
  assert.equal(extractPipelineStatusKind(payload), "Committed");
});

test("extractPipelineStatusKind falls back to direct status string", () => {
  const payload = { status: "Rejected" };
  assert.equal(extractPipelineStatusKind(payload), "Rejected");
});

test("extractPipelineStatusKind returns null when status missing", () => {
  assert.equal(extractPipelineStatusKind({}), null);
  assert.equal(extractPipelineStatusKind(null), null);
});

test("decodePdpCommitmentHeader decodes base64 payloads", () => {
  const payload = Buffer.from([0xde, 0xad, 0xbe, 0xef]);
  const headers = { "sora-pdp-commitment": payload.toString("base64") };
  const decoded = decodePdpCommitmentHeader(headers);
  assert.ok(decoded instanceof Uint8Array);
  assert.deepEqual(Buffer.from(decoded ?? []), payload);
});

test("decodePdpCommitmentHeader handles Headers objects", () => {
  const headers = new Headers();
  headers.set("Sora-PDP-Commitment", Buffer.from([0xcd]).toString("base64"));
  const decoded = decodePdpCommitmentHeader(headers);
  assert.deepEqual(Buffer.from(decoded ?? []), Buffer.from([0xcd]));
});

test("decodePdpCommitmentHeader throws on invalid base64 strings", () => {
  assert.throws(
    () => decodePdpCommitmentHeader({ "sora-pdp-commitment": "###" }),
    /Failed to decode Sora-PDP-Commitment header/,
  );
});

test("decodePdpCommitmentHeader returns null when header absent", () => {
  assert.equal(decodePdpCommitmentHeader(null), null);
  assert.equal(decodePdpCommitmentHeader({}), null);
});

test("waitForTransactionStatus rejects non-object options", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200 }) });
  const hashHex = "aa".repeat(32);
  await assert.rejects(
    () => client.waitForTransactionStatus(hashHex, "invalid"),
    /waitForTransactionStatus options must be a plain object/,
  );
});

test("waitForTransactionStatus enforces numeric poll options", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200 }) });
  const hashHex = "bb".repeat(32);
  await assert.rejects(
    () => client.waitForTransactionStatus(hashHex, { intervalMs: -1 }),
    /waitForTransactionStatus options\.intervalMs must be a non-negative integer/,
  );
  await assert.rejects(
    () => client.waitForTransactionStatus(hashHex, { timeoutMs: -5 }),
    /waitForTransactionStatus options\.timeoutMs must be a non-negative integer/,
  );
  await assert.rejects(
    () => client.waitForTransactionStatus(hashHex, { maxAttempts: 0 }),
    /waitForTransactionStatus options\.maxAttempts must be a positive integer/,
  );
});

test("waitForTransactionStatus rejects unsupported options", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200 }) });
  const hashHex = "cc".repeat(32);
  await assert.rejects(
    () => client.waitForTransactionStatus(hashHex, { intervalMs: 0, extra: true }),
    /waitForTransactionStatus options contains unsupported fields: extra/,
  );
});

test("waitForTransactionStatus validates signal option type", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200 }) });
  const hashHex = "cd".repeat(32);
  await assert.rejects(
    () =>
      client.waitForTransactionStatus(hashHex, {
        // @ts-expect-error runtime validation should reject incorrect signal
        signal: {},
      }),
    /waitForTransactionStatus options\.signal must be an AbortSignal/,
  );
});

test("waitForTransactionStatus enforces onStatus callback type", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200 }) });
  await assert.rejects(
    () => client.waitForTransactionStatus("cc".repeat(32), { onStatus: "noop" }),
    /waitForTransactionStatus options\.onStatus must be a function/,
  );
});

test("waitForTransactionStatus rejects invalid hash literals", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200 }) });
  await assert.rejects(
    () => client.waitForTransactionStatus("deadbeef", { maxAttempts: 1 }),
    /waitForTransactionStatus\.hashHex/,
  );
});

test("waitForTransactionStatus resolves on nested committed status", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200 }) });

  const txHash = "ef".repeat(32);
  const requestHash = "dd".repeat(32);
  const statuses = [
    { kind: "Transaction", content: { hash: txHash, status: { kind: "Pending", content: null } } },
    { kind: "Transaction", content: { hash: txHash, status: { kind: "Committed", content: "YQ==" } } },
  ];
  client.getTransactionStatus = async () => statuses.shift();

  const observed = [];
  const result = await client.waitForTransactionStatus(requestHash, {
    intervalMs: 0,
    timeoutMs: 50,
    onStatus: (status, payload, attempt) => observed.push({ status, attempt, payload }),
  });

  assert.equal(observed.length, 2);
  assert.deepEqual(observed.map((entry) => entry.status), ["Pending", "Committed"]);
  assert.deepEqual(result, {
    kind: "Transaction",
    content: { hash: txHash, status: { kind: "Committed", content: "YQ==" } },
  });
});

test("waitForTransactionStatus forwards signal and aborts polling", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200 }) });
  const txHash = "66".repeat(32);
  const controller = new AbortController();
  let attempts = 0;
  let seenSignal = null;
  client.getTransactionStatus = async (_hashHex, options = {}) => {
    attempts += 1;
    seenSignal = options.signal ?? null;
    controller.abort(new Error("stop polling"));
    return {
      kind: "Transaction",
      content: { hash: txHash, status: { kind: "Pending", content: null } },
    };
  };

  await assert.rejects(
    () =>
      client.waitForTransactionStatus(txHash, {
        signal: controller.signal,
        intervalMs: 0,
        timeoutMs: null,
        maxAttempts: 10,
      }),
    (error) => error instanceof Error && error.message === "stop polling",
  );
  assert.equal(attempts, 1);
  assert.equal(seenSignal, controller.signal);
});

test("waitForTransactionStatusTyped normalises payload", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200 }) });
  const txHash = "11".repeat(32);
  client.waitForTransactionStatus = async () => ({
    kind: "Transaction",
    content: { hash: txHash, status: { kind: "Committed" } },
  });
  const typed = await client.waitForTransactionStatusTyped(txHash, { intervalMs: 0 });
  assert.equal(typed?.hashHex, txHash);
  assert.equal(typed?.status?.kind, "Committed");
});

test("waitForTransactionStatus rejects on failure status", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200 }) });
  const rejectionHash = "22".repeat(32);
  client.getTransactionStatus = async () => ({
    kind: "Transaction",
    content: { hash: rejectionHash, status: { kind: "Rejected", content: null } },
  });

  await assert.rejects(
    () => client.waitForTransactionStatus(rejectionHash, { intervalMs: 0, maxAttempts: 1 }),
    (error) => error instanceof TransactionStatusError && error.status === "Rejected",
  );
});

test("waitForTransactionStatus surfaces rejection reason on failure status", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200 }) });
  const rejectionHash = "23".repeat(32);
  const rejectionReason = "build_claim_missing";
  client.getTransactionStatus = async () => ({
    kind: "Transaction",
    content: {
      hash: rejectionHash,
      status: {
        kind: "Rejected",
        content: null,
        rejection_reason: rejectionReason,
      },
    },
  });

  await assert.rejects(
    () => client.waitForTransactionStatus(rejectionHash, { intervalMs: 0, maxAttempts: 1 }),
    (error) =>
      error instanceof TransactionStatusError
      && error.status === "Rejected"
      && error.rejectionReason === rejectionReason
      && String(error.message).includes(`reason=${rejectionReason}`),
  );
});

test("waitForTransactionStatus respects maxAttempts", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200 }) });
  let calls = 0;
  const pendingHash = "33".repeat(32);
  client.getTransactionStatus = async () => {
    calls += 1;
    return { kind: "Transaction", content: { hash: pendingHash, status: { kind: "Pending", content: null } } };
  };

  await assert.rejects(
    () =>
      client.waitForTransactionStatus(pendingHash, {
        intervalMs: 0,
        maxAttempts: 2,
        timeoutMs: null,
      }),
    (error) => error instanceof TransactionTimeoutError && error.attempts === 2,
  );
  assert.equal(calls, 2);
});

test("waitForTransactionStatus enforces timeoutMs", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200 }) });
  const pendingHash = "44".repeat(32);
  client.getTransactionStatus = async () => ({
    kind: "Transaction",
    content: { hash: pendingHash, status: { kind: "Pending", content: null } },
  });

  await assert.rejects(
    () =>
      client.waitForTransactionStatus(pendingHash, {
        intervalMs: 0,
        timeoutMs: 0,
      }),
    (error) => error instanceof TransactionTimeoutError,
  );
});

test("submitTransactionAndWait delegates to submitTransaction + waitForTransactionStatus", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 200 }),
  });

  const payload = Buffer.from([0xde, 0xad, 0xbe, 0xef]);
  let submittedPayload = null;
  let waitArgs = null;
  const finalHash = "55".repeat(32);
  const expectedResult = {
    kind: "Transaction",
    content: { hash: finalHash, status: { kind: "Committed", content: null } },
  };

  client.submitTransaction = async (body) => {
    submittedPayload = body;
  };
  client.waitForTransactionStatus = async (hashHex, pollOptions) => {
    waitArgs = { hashHex, pollOptions };
    return expectedResult;
  };

  const result = await client.submitTransactionAndWait(payload, {
    hashHex: finalHash,
    timeoutMs: 500,
    successStatuses: ["Committed"],
  });

  assert.strictEqual(submittedPayload, payload);
  assert.deepEqual(waitArgs, {
    hashHex: finalHash,
    pollOptions: {
      timeoutMs: 500,
      successStatuses: ["Committed"],
    },
  });
  assert.strictEqual(result, expectedResult);
});

test("submitTransactionAndWait enforces hashHex option", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 200 }),
  });
  const dummy = Buffer.from([0]);
  await assert.rejects(
    () => client.submitTransactionAndWait(dummy),
    /submitTransactionAndWait options must be a plain object/,
  );
  await assert.rejects(
    () => client.submitTransactionAndWait(dummy, {}),
    /options\.hashHex must be a hex string/,
  );
});

test("submitTransactionAndWaitTyped normalises the final payload", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 200 }),
  });
  const typedHash = "66".repeat(32);
  const pipelineStatus = {
    kind: "Transaction",
    content: { hash: typedHash, status: { kind: "Approved" } },
  };
  client.submitTransactionAndWait = async () => pipelineStatus;
  const typed = await client.submitTransactionAndWaitTyped(Buffer.from([0]), {
    hashHex: typedHash,
  });
  assert.equal(typed?.hashHex, typedHash);
  assert.equal(typed?.status?.kind, "Approved");
});

test("getHealth requests JSON snapshot", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/health`);
    assert.equal(init.headers.Accept, "application/json");
    return createResponse({
      status: 200,
      jsonData: { status: "healthy" },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.getHealth();
  assert.deepEqual(payload, { status: "healthy" });
});

test("getHealth returns null for non-JSON responses", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      textBody: "Healthy\r\n",
      headers: { "content-type": "text/plain; charset=utf-8" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.getHealth();
  assert.equal(payload, null);
});

test("getHealth returns null when the body is empty", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: null,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.getHealth();
  assert.equal(payload, null);
});

test("getHealth rejects non-object options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(() => client.getHealth("bad-options"), (error) =>
    expectValidationErrorFixture(error, "getHealth_options_invalid"),
  );
});

test("health endpoints reject unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(() => client.getHealth({ extra: true }), (error) =>
    expectValidationErrorFixture(error, "getHealth_options_extra"),
  );
  await assert.rejects(() => client.getStatusSnapshot({ note: "nope" }), (error) =>
    expectValidationErrorFixture(error, "getStatusSnapshot_options_extra"),
  );
});

test("getStatusSnapshot rejects invalid signals", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () => client.getStatusSnapshot({ signal: "not-a-signal" }),
    (error) => expectValidationErrorFixture(error, "getStatusSnapshot_invalid_signal"),
  );
});

test("getSumeragiStatus validates options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(() => client.getSumeragiStatus(5), (error) =>
    expectValidationErrorFixture(error, "getSumeragiStatus_options_invalid"),
  );
  await assert.rejects(
    () => client.getSumeragiStatus({ signal: "nope" }),
    (error) => expectValidationErrorFixture(error, "getSumeragiStatus_invalid_signal"),
  );
  await assert.rejects(() => client.getSumeragiStatus({ extra: true }), (error) =>
    expectValidationErrorFixture(error, "getSumeragiStatus_options_extra"),
  );
});

test("getSumeragiStatus fetches consensus metrics", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/sumeragi/status`);
    assert.equal(init.headers.Accept, "application/json");
    return createResponse({
      status: 200,
      jsonData: {
        leader_index: 1,
        membership: { height: 11, view: 3, epoch: 2, view_hash: "deadbeef" },
        lane_commitments: [
          {
            block_height: 11,
            lane_id: 2,
            tx_count: 3,
            total_chunks: 4,
            rbc_bytes_total: 512,
            teu_total: 120,
            block_hash: "cafebabe",
          },
        ],
        dataspace_commitments: [
          {
            block_height: 11,
            lane_id: 2,
            dataspace_id: 7,
            tx_count: 1,
            total_chunks: 2,
            rbc_bytes_total: 128,
            teu_total: 32,
            block_hash: "cafedead",
          },
        ],
        lane_governance: [
          {
            lane_id: 2,
            alias: "alpha-lane",
            dataspace_id: 7,
            visibility: "public",
            storage_profile: "full_replica",
            governance: "parliament",
            manifest_required: true,
            manifest_ready: true,
            manifest_path: "/etc/iroha/lanes/alpha.json",
            validator_ids: ["alice@test", "bob@test"],
            quorum: 2,
            protected_namespaces: ["finance"],
        runtime_upgrade: {
          allow: true,
          require_metadata: true,
          metadata_key: "upgrade_id",
          allowed_ids: ["alpha-upgrade"],
        },
        privacy_commitments: [
          {
            id: 7,
            scheme: "merkle",
            merkle: { root: "0xaaaabbbb", max_depth: 16 },
            snark: null,
          },
          {
            id: 9,
            scheme: "snark",
            merkle: null,
            snark: {
              circuit_id: 5,
              verifying_key_digest: "0x11112222",
              statement_hash: "0x33334444",
              proof_hash: "0x55556666",
            },
          },
        ],
      },
    ],
        lane_settlement_commitments: [
          {
            block_height: 11,
            lane_id: 2,
            dataspace_id: 7,
            tx_count: 1,
            total_local_micro: 10,
            total_xor_due_micro: 5,
            total_xor_after_haircut_micro: 4,
            total_xor_variance_micro: 1,
            swap_metadata: {
              epsilon_bps: 12,
              twap_window_seconds: 30,
              liquidity_profile: "tier2",
              twap_local_per_xor: "1.0",
              volatility_class: "elevated",
            },
            receipts: [
              {
                source_id: "0xabc",
                local_amount_micro: 10,
                xor_due_micro: 5,
                xor_after_haircut_micro: 4,
                xor_variance_micro: 1,
                timestamp_ms: 1700,
              },
            ],
          },
        ],
        lane_relay_envelopes: [
          {
            lane_id: 2,
            dataspace_id: 7,
            block_height: 11,
            block_header: { height: 11, view: 3 },
            qc: { subject_block_hash: "feedface" },
            da_commitment_hash: "0xcafe",
            settlement_commitment: {
              block_height: 11,
              lane_id: 2,
              dataspace_id: 7,
              tx_count: 1,
              total_local_micro: 10,
              total_xor_due_micro: 5,
              total_xor_after_haircut_micro: 4,
              total_xor_variance_micro: 1,
              swap_metadata: null,
              receipts: [
                {
                  source_id: "0xdef",
                  local_amount_micro: 10,
                  xor_due_micro: 5,
                  xor_after_haircut_micro: 4,
                  xor_variance_micro: 1,
                  timestamp_ms: 1701,
                },
              ],
            },
            settlement_hash: "0x1234",
            rbc_bytes_total: 256,
          },
        ],
        da_reschedule_total: 3,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.getSumeragiStatus();
  assert.deepEqual(payload, {
    leader_index: 1,
    membership: { height: 11, view: 3, epoch: 2, view_hash: "deadbeef" },
    lane_commitments: [
      {
        block_height: 11,
        lane_id: 2,
        tx_count: 3,
        total_chunks: 4,
        rbc_bytes_total: 512,
        teu_total: 120,
        block_hash: "cafebabe",
      },
    ],
    dataspace_commitments: [
      {
        block_height: 11,
        lane_id: 2,
        dataspace_id: 7,
        tx_count: 1,
        total_chunks: 2,
        rbc_bytes_total: 128,
        teu_total: 32,
        block_hash: "cafedead",
      },
    ],
    lane_governance: [
      {
        lane_id: 2,
        alias: "alpha-lane",
        dataspace_id: 7,
        visibility: "public",
        storage_profile: "full_replica",
        governance: "parliament",
        manifest_required: true,
        manifest_ready: true,
        manifest_path: "/etc/iroha/lanes/alpha.json",
        validator_ids: ["alice@test", "bob@test"],
        quorum: 2,
        protected_namespaces: ["finance"],
        runtime_upgrade: {
          allow: true,
          require_metadata: true,
          metadata_key: "upgrade_id",
          allowed_ids: ["alpha-upgrade"],
        },
        privacy_commitments: [
          {
            id: 7,
            scheme: "merkle",
            merkle: { root: "0xaaaabbbb", max_depth: 16 },
            snark: null,
          },
          {
            id: 9,
            scheme: "snark",
            merkle: null,
            snark: {
              circuit_id: 5,
              verifying_key_digest: "0x11112222",
              statement_hash: "0x33334444",
              proof_hash: "0x55556666",
            },
          },
        ],
      },
    ],
    lane_settlement_commitments: [
      {
        block_height: 11,
        lane_id: 2,
        dataspace_id: 7,
        tx_count: 1,
        total_local_micro: 10,
        total_xor_due_micro: 5,
        total_xor_after_haircut_micro: 4,
        total_xor_variance_micro: 1,
        swap_metadata: {
          epsilon_bps: 12,
          twap_window_seconds: 30,
          liquidity_profile: "tier2",
          twap_local_per_xor: "1.0",
          volatility_class: "elevated",
        },
        receipts: [
          {
            source_id: "0xabc",
            local_amount_micro: 10,
            xor_due_micro: 5,
            xor_after_haircut_micro: 4,
            xor_variance_micro: 1,
            timestamp_ms: 1700,
          },
        ],
      },
    ],
    lane_relay_envelopes: [
      {
        lane_id: 2,
        dataspace_id: 7,
        block_height: 11,
        block_header: { height: 11, view: 3 },
        qc: { subject_block_hash: "feedface" },
        da_commitment_hash: "0xcafe",
        settlement_commitment: {
          block_height: 11,
          lane_id: 2,
          dataspace_id: 7,
          tx_count: 1,
          total_local_micro: 10,
          total_xor_due_micro: 5,
          total_xor_after_haircut_micro: 4,
          total_xor_variance_micro: 1,
          swap_metadata: null,
          receipts: [
            {
              source_id: "0xdef",
              local_amount_micro: 10,
              xor_due_micro: 5,
              xor_after_haircut_micro: 4,
              xor_variance_micro: 1,
              timestamp_ms: 1701,
            },
          ],
        },
        settlement_hash: "0x1234",
        rbc_bytes_total: 256,
      },
    ],
    da_reschedule_total: 3,
  });
});

test("getSumeragiStatusTyped normalizes governance seals", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/sumeragi/status`);
    assert.equal(init.headers.Accept, "application/json");
    return createResponse({
      status: 200,
      jsonData: {
        mode_tag: "iroha2-consensus::permissioned-sumeragi@v1",
        staged_mode_tag: "iroha2-consensus::npos-sumeragi@v1",
        staged_mode_activation_height: "10",
        mode_activation_lag_blocks: 2,
        consensus_caps: {
          collectors_k: "2",
          redundant_send_r: "1",
          da_enabled: true,
          rbc_chunk_max_bytes: "1024",
          rbc_session_ttl_ms: 5000,
          rbc_store_max_sessions: "64",
          rbc_store_soft_sessions: "32",
          rbc_store_max_bytes: "4096",
          rbc_store_soft_bytes: "2048",
        },
        commit_qc: {
          height: "41",
          view: "2",
          epoch: "5",
          block_hash: "0xabc",
          validator_set_hash: "0xdef",
          validator_set_len: "4",
          signatures_total: "3",
        },
        commit_quorum: {
          height: "41",
          view: "2",
          block_hash: "0xabc",
          signatures_present: "3",
          signatures_counted: "3",
          signatures_set_b: "2",
          signatures_required: "3",
          last_updated_ms: "1700",
        },
        leader_index: "5",
        lane_commitments: [
          {
            block_height: "42",
            lane_id: "7",
            tx_count: 3,
            total_chunks: 4,
            rbc_bytes_total: 256,
            teu_total: 64,
            block_hash: "feedface",
          },
        ],
        dataspace_commitments: [
          {
            block_height: "42",
            lane_id: "7",
            dataspace_id: 9,
            tx_count: 1,
            total_chunks: 1,
            rbc_bytes_total: 96,
            teu_total: 16,
            block_hash: "facedead",
          },
        ],
        lane_governance: [
          {
            lane_id: "7",
            alias: "archive",
            dataspace_id: "9",
            visibility: "public",
            storage_profile: "full_replica",
            manifest_required: true,
            manifest_ready: false,
            validator_ids: ["alice@test"],
            protected_namespaces: ["finance"],
            privacy_commitments: [
              {
                id: "4",
                scheme: "merkle",
                merkle: { root: "0xffff", max_depth: "12" },
              },
              {
                id: 5,
                scheme: "snark",
                snark: {
                  circuit_id: "8",
                  verifying_key_digest: "0xaaaa",
                  statement_hash: "0xbbbb",
                  proof_hash: "0xcccc",
                },
              },
            ],
          },
        ],
        lane_governance_sealed_total: "2",
        lane_governance_sealed_aliases: ["archive", "payments"],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.getSumeragiStatusTyped();
  assert.equal(payload.mode_tag, "iroha2-consensus::permissioned-sumeragi@v1");
  assert.equal(payload.staged_mode_tag, "iroha2-consensus::npos-sumeragi@v1");
  assert.equal(payload.staged_mode_activation_height, 10);
  assert.equal(payload.mode_activation_lag_blocks, 2);
  assert.ok(payload.consensus_caps);
  assert.equal(payload.consensus_caps.collectors_k, 2);
  assert.equal(payload.consensus_caps.rbc_chunk_max_bytes, 1024);
  assert.deepEqual(payload.commit_qc, {
    height: 41,
    view: 2,
    epoch: 5,
    block_hash: "0xabc",
    validator_set_hash: "0xdef",
    validator_set_len: 4,
    signatures_total: 3,
  });
  assert.deepEqual(payload.commit_quorum, {
    height: 41,
    view: 2,
    block_hash: "0xabc",
    signatures_present: 3,
    signatures_counted: 3,
    signatures_set_b: 2,
    signatures_required: 3,
    last_updated_ms: 1700,
  });
  assert.equal(payload.leader_index, "5");
  assert.deepEqual(payload.lane_commitments, [
    {
      block_height: 42,
      lane_id: 7,
      tx_count: 3,
      total_chunks: 4,
      rbc_bytes_total: 256,
      teu_total: 64,
      block_hash: "feedface",
    },
  ]);
  assert.deepEqual(payload.dataspace_commitments, [
    {
      block_height: 42,
      lane_id: 7,
      dataspace_id: 9,
      tx_count: 1,
      total_chunks: 1,
      rbc_bytes_total: 96,
      teu_total: 16,
      block_hash: "facedead",
    },
  ]);
  assert.deepEqual(payload.lane_governance, [
    {
      lane_id: 7,
      alias: "archive",
      dataspace_id: 9,
      visibility: "public",
      storage_profile: "full_replica",
      governance: null,
      manifest_required: true,
      manifest_ready: false,
      manifest_path: null,
      validator_ids: ["alice@test"],
      quorum: null,
      protected_namespaces: ["finance"],
      runtime_upgrade: null,
      privacy_commitments: [
        {
          id: 4,
          scheme: "merkle",
          merkle: { root: "0xffff", max_depth: 12 },
          snark: null,
        },
        {
          id: 5,
          scheme: "snark",
          merkle: null,
          snark: {
            circuit_id: 8,
            verifying_key_digest: "0xaaaa",
            statement_hash: "0xbbbb",
            proof_hash: "0xcccc",
          },
        },
      ],
    },
  ]);
  assert.equal(payload.lane_governance_sealed_total, 2);
  assert.deepEqual(payload.lane_governance_sealed_aliases, ["archive", "payments"]);
});

test("getSumeragiStatusTyped parses settlement and relay envelopes", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        mode_tag: "iroha2-consensus::permissioned-sumeragi@v1",
        lane_settlement_commitments: [
          {
            block_height: "21",
            lane_id: "4",
            dataspace_id: "9",
            tx_count: 2,
            total_local_micro: 42,
            total_xor_due_micro: 21,
            total_xor_after_haircut_micro: 20,
            total_xor_variance_micro: 1,
            swap_metadata: {
              epsilon_bps: "5",
              twap_window_seconds: 60,
              liquidity_profile: "tier1",
              twap_local_per_xor: "2.5",
              volatility_class: "stable",
            },
            receipts: [
              {
                source_id: "0xaaaa",
                local_amount_micro: 42,
                xor_due_micro: 21,
                xor_after_haircut_micro: 20,
                xor_variance_micro: 1,
                timestamp_ms: 1700,
              },
            ],
          },
        ],
        lane_relay_envelopes: [
          {
            lane_id: 4,
            dataspace_id: 9,
            block_height: 21,
            block_header: { height: 21 },
            qc: { subject_block_hash: "abcd" },
            da_commitment_hash: null,
            settlement_commitment: {
              block_height: 21,
              lane_id: 4,
              dataspace_id: 9,
              tx_count: 2,
              total_local_micro: 42,
              total_xor_due_micro: 21,
              total_xor_after_haircut_micro: 20,
              total_xor_variance_micro: 1,
              receipts: [
                {
                  source_id: "0xbbbb",
                  local_amount_micro: 10,
                  xor_due_micro: 5,
                  xor_after_haircut_micro: 4,
                  xor_variance_micro: 1,
                  timestamp_ms: 1701,
                },
              ],
            },
            settlement_hash: "0xfeed",
            rbc_bytes_total: "64",
            manifest_root: "0xcafe",
            fastpq_proof: {
              proof_digest: "0xdeadbeef",
              verified_at_height: "22",
            },
          },
        ],
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const status = await client.getSumeragiStatusTyped();
  assert.equal(status.lane_settlement_commitments[0].total_local_micro, 42);
  assert.equal(status.lane_relay_envelopes[0].settlement_hash, "0xfeed");
  assert.equal(status.lane_relay_envelopes[0].rbc_bytes_total, 64);
  assert.equal(status.lane_relay_envelopes[0].manifest_root, "0xcafe");
  assert.equal(status.lane_relay_envelopes[0].fastpq_proof.proof_digest, "0xdeadbeef");
  assert.equal(status.lane_relay_envelopes[0].fastpq_proof.verified_at_height, 22);
});

test("getSumeragiStatusTyped rejects invalid relay settlement hash", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        mode_tag: "iroha2-consensus::permissioned-sumeragi@v1",
        lane_relay_envelopes: [
          {
            lane_id: 1,
            dataspace_id: 1,
            block_height: 1,
            block_header: { height: 1 },
            settlement_commitment: {
              block_height: 1,
              lane_id: 1,
              dataspace_id: 1,
              tx_count: 0,
              total_local_micro: 0,
              total_xor_due_micro: 0,
              total_xor_after_haircut_micro: 0,
              total_xor_variance_micro: 0,
              receipts: [],
            },
            settlement_hash: 42,
            rbc_bytes_total: 0,
          },
        ],
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(() => client.getSumeragiStatusTyped(), /settlement_hash/);
});

test("getSumeragiStatusTyped rejects invalid relay fastpq_proof digest", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        mode_tag: "iroha2-consensus::permissioned-sumeragi@v1",
        lane_relay_envelopes: [
          {
            lane_id: 1,
            dataspace_id: 1,
            block_height: 1,
            block_header: { height: 1 },
            settlement_commitment: {
              block_height: 1,
              lane_id: 1,
              dataspace_id: 1,
              tx_count: 0,
              total_local_micro: 0,
              total_xor_due_micro: 0,
              total_xor_after_haircut_micro: 0,
              total_xor_variance_micro: 0,
              receipts: [],
            },
            settlement_hash: "0x00",
            rbc_bytes_total: 0,
            fastpq_proof: { proof_digest: 42 },
          },
        ],
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(() => client.getSumeragiStatusTyped(), /fastpq_proof.proof_digest/);
});

test("getSumeragiPacemaker returns null when gated and decodes payload otherwise", async () => {
  const snapshots = [
    createResponse({ status: 403, jsonData: { ok: false }, headers: { "content-type": "application/json" } }),
    createResponse({
      status: 200,
      jsonData: {
        backoff_ms: "100",
        rtt_floor_ms: "25",
        jitter_ms: "5",
        backoff_multiplier: "2",
        rtt_floor_multiplier: "1",
        max_backoff_ms: "500",
        jitter_frac_permille: "15",
        round_elapsed_ms: "42",
        view_timeout_target_ms: "200",
        view_timeout_remaining_ms: "150",
      },
      headers: { "content-type": "application/json" },
    }),
  ];
  let call = 0;
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/sumeragi/pacemaker`);
    assert.equal(init.headers.Accept, "application/json");
    call += 1;
    return snapshots[call - 1];
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const forbidden = await client.getSumeragiPacemaker();
  assert.equal(forbidden, null);
  const snapshot = await client.getSumeragiPacemaker();
  assert.deepEqual(snapshot, {
    backoff_ms: 100,
    rtt_floor_ms: 25,
    jitter_ms: 5,
    backoff_multiplier: 2,
    rtt_floor_multiplier: 1,
    max_backoff_ms: 500,
    jitter_frac_permille: 15,
    round_elapsed_ms: 42,
    view_timeout_target_ms: 200,
    view_timeout_remaining_ms: 150,
  });
});

test("getSumeragiPacemaker rejects invalid AbortSignal option", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 403, jsonData: {}, headers: { "content-type": "application/json" } }),
  });
  await assert.rejects(
    () =>
      client.getSumeragiPacemaker({
        // @ts-expect-error runtime validation should reject incorrect signal
        signal: {},
      }),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(error.message, /getSumeragiPacemaker options\.signal must be an AbortSignal/);
      return true;
    },
  );
});

test("Sumeragi snapshot endpoints reject unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not be invoked for option validation");
    },
  });
  const cases = [
    [
      "getSumeragiPacemaker",
      () => client.getSumeragiPacemaker({ extra: true }),
      "extra",
    ],
    [
      "getSumeragiQc",
      () => client.getSumeragiQc({ unexpected: "nope" }),
      "unexpected",
    ],
    [
      "getSumeragiPhases",
      () => client.getSumeragiPhases({ note: "bad" }),
      "note",
    ],
    [
      "getSumeragiBlsKeys",
      () => client.getSumeragiBlsKeys({ window: 1 }),
      "window",
    ],
    [
      "getSumeragiLeader",
      () => client.getSumeragiLeader({ invalid: true }),
      "invalid",
    ],
    [
      "getSumeragiCollectors",
      () => client.getSumeragiCollectors({ debug: "flag" }),
      "debug",
    ],
  ];
  for (const [label, invoke, field] of cases) {
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(invoke, (error) => {
      assert(error instanceof TypeError);
      assert.match(
        error.message,
        new RegExp(`${label} options contains unsupported fields: ${field}`),
      );
      return true;
    });
  }
});

test("getSumeragiQc fetches QC snapshot", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/sumeragi/qc`);
    assert.equal(init.headers.Accept, "application/json");
    return createResponse({
      status: 200,
      jsonData: {
        highest_qc: { height: "10", view: "2", subject_block_hash: "abc" },
        locked_qc: { height: "9", view: "1", subject_block_hash: null },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const qc = await client.getSumeragiQc();
  assert.deepEqual(qc, {
    highest_qc: { height: 10, view: 2, subject_block_hash: "abc" },
    locked_qc: { height: 9, view: 1, subject_block_hash: null },
  });
});

test("getSumeragiCommitQc fetches commit QC record", async () => {
  const hashHex = "aa".repeat(32);
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/sumeragi/commit_qc/${hashHex}`);
    assert.equal(init.headers.Accept, "application/json");
    return createResponse({
      status: 200,
      jsonData: {
        subject_block_hash: hashHex,
        commit_qc: {
          phase: "Commit",
          parent_state_root: "bb".repeat(32),
          post_state_root: "cc".repeat(32),
          height: "12",
          view: "3",
          epoch: "4",
          mode_tag: "iroha2-consensus::permissioned-sumeragi@v1",
          validator_set_hash: "dd".repeat(32),
          validator_set_hash_version: 1,
          validator_set: ["alice@test", "bob@test"],
          signers_bitmap: "0a",
          bls_aggregate_signature: "ff",
        },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const record = await client.getSumeragiCommitQc(`0x${hashHex}`);
  assert.equal(record.subject_block_hash, hashHex);
  assert.equal(record.commit_qc?.parent_state_root, "bb".repeat(32));
  assert.equal(record.commit_qc?.validator_set.length, 2);
});

test("getSumeragiPhases fetches latency metrics", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/sumeragi/phases`);
    assert.equal(init.headers.Accept, "application/json");
    return createResponse({
      status: 200,
      jsonData: {
        propose_ms: "5",
        collect_da_ms: "6",
        collect_prevote_ms: "7",
        collect_precommit_ms: "8",
        collect_aggregator_ms: "9",
        commit_ms: "12",
        pipeline_total_ms: "58",
        collect_aggregator_gossip_total: "3",
        block_created_dropped_by_lock_total: "1",
        block_created_hint_mismatch_total: "2",
        block_created_proposal_mismatch_total: "0",
        ema_ms: {
          propose_ms: "4",
          collect_da_ms: "5",
          collect_prevote_ms: "6",
          collect_precommit_ms: "7",
          collect_aggregator_ms: "8",
          commit_ms: "11",
          pipeline_total_ms: "50",
        },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const phases = await client.getSumeragiPhases();
  assert.equal(phases.collect_aggregator_ms, 9);
  assert.equal(phases.ema_ms.pipeline_total_ms, 50);
});

test("getSumeragiBlsKeys returns network map", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/sumeragi/bls_keys`);
    assert.equal(init.headers.Accept, "application/json");
    return createResponse({
      status: 200,
      jsonData: {
        "ed0120...01": null,
        "bls1...ff": "bls1...ff",
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const mapping = await client.getSumeragiBlsKeys();
  assert.deepEqual(mapping, {
    "ed0120...01": null,
    "bls1...ff": "bls1...ff",
  });
});

test("getSumeragiBlsKeys rejects malformed payloads", async () => {
  const fetchImpl = async () => {
    return createResponse({
      status: 200,
      jsonData: { "ed0120...01": 42 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(() => client.getSumeragiBlsKeys(), /sumeragi BLS key/);
});

test("getSumeragiLeader fetches leader and PRF context", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/sumeragi/leader`);
    assert.equal(init.headers.Accept, "application/json");
    return createResponse({
      status: 200,
      jsonData: {
        leader_index: "3",
        prf: { height: "10", view: "2", epoch_seed: "seed" },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const leader = await client.getSumeragiLeader();
  assert.equal(leader.leader_index, 3);
  assert.equal(leader.prf.epoch_seed, "seed");
});

test("getSumeragiCollectors fetches collector plan", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/sumeragi/collectors`);
    assert.equal(init.headers.Accept, "application/json");
    return createResponse({
      status: 200,
      jsonData: {
        consensus_mode: "Permissioned",
        mode: "Permissioned",
        topology_len: "5",
        min_votes_for_commit: "4",
        proxy_tail_index: "2",
        height: "11",
        view: "3",
        collectors_k: "2",
        redundant_send_r: "1",
        epoch_seed: "cafebabe",
        collectors: [
          { index: "0", peer_id: "alice@test" },
          { index: "2", peer_id: "bob@test" },
        ],
        prf: { height: "11", view: "3", epoch_seed: "cafebabe" },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const plan = await client.getSumeragiCollectors();
  assert.equal(plan.collectors.length, 2);
  assert.equal(plan.collectors_k, 2);
});

test("getSumeragiParams fetches on-chain parameters", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/sumeragi/params`);
    assert.equal(init.headers.Accept, "application/json");
    return createResponse({
      status: 200,
      jsonData: {
        block_time_ms: "1000",
        commit_time_ms: "400",
        max_clock_drift_ms: "50",
        collectors_k: "3",
        redundant_send_r: "1",
        da_enabled: "false",
        next_mode: "Npos",
        mode_activation_height: "5000",
        chain_height: "4200",
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const params = await client.getSumeragiParams();
  assert.equal(params.block_time_ms, 1000);
  assert.equal(params.next_mode, "Npos");
  assert.equal(params.da_enabled, false);
});

test("Sumeragi params/telemetry reject unsupported options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: () => {
      throw new Error("fetch should not be called for validation failures");
    },
  });
  await assert.rejects(
    client.getSumeragiParams({ unexpected: true }),
    /getSumeragiParams options contains unsupported fields: unexpected/,
  );
  await assert.rejects(
    client.getSumeragiTelemetry({ extra: "value" }),
    /getSumeragiTelemetry options contains unsupported fields: extra/,
  );
});

test("getSumeragiTelemetryTyped normalizes availability, latency, backlog, and VRF data", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/sumeragi/telemetry`);
    assert.equal(init.headers.Accept, "application/json");
    return createResponse({
      status: 200,
      jsonData: {
        availability: {
          total_votes_ingested: "42",
          collectors: [
            { collector_idx: "0", peer_id: "alice@test", votes_ingested: "20" },
            { collector_idx: "1", peer_id: "bob@test", votes_ingested: "22" },
          ],
        },
        qc_latency_ms: [{ kind: "CollectPrepare", last_ms: "15" }],
        rbc_backlog: {
          pending_sessions: "2",
          total_missing_chunks: "8",
          max_missing_chunks: "5",
        },
        vrf: {
          found: true,
          epoch: "12",
          finalized: false,
          seed_hex: "cafebabe",
          epoch_length: "16",
          commit_deadline_offset: "3",
          reveal_deadline_offset: "4",
          roster_len: "7",
          updated_at_height: "1024",
          participants_total: "9",
          commitments_total: "8",
          reveals_total: "7",
          late_reveals_total: "1",
          committed_no_reveal: ["1", "2"],
          no_participation: ["3"],
          late_reveals: [{ signer: "carol@test", noted_at_height: "1025" }],
        },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const snapshot = await client.getSumeragiTelemetryTyped();
  assert.equal(snapshot.availability.total_votes_ingested, 42);
  assert.equal(snapshot.availability.collectors[1].peer_id, "bob@test");
  assert.equal(snapshot.qc_latency_ms[0].last_ms, 15);
  assert.equal(snapshot.rbc_backlog.max_missing_chunks, 5);
  assert.equal(snapshot.vrf.seed_hex, "cafebabe");
  assert.deepEqual(snapshot.vrf.committed_no_reveal, [1, 2]);
  assert.equal(snapshot.vrf.late_reveals[0].noted_at_height, 1025);
});

test("getSumeragiTelemetryTyped rejects malformed telemetry payloads", async () => {
  const fetchImpl = async () => {
    return createResponse({
      status: 200,
      jsonData: { availability: null },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getSumeragiTelemetryTyped(),
    /sumeragi telemetry\.availability/,
  );
});

test("getStatusSnapshot normalizes payload and tracks metrics", async () => {
  const payloads = [
    {
      peers: "5",
      queue_size: 3,
      commit_time_ms: 420,
      da_reschedule_total: "7",
      txs_approved: 100,
      txs_rejected: 2,
      view_changes: 1,
      governance: {
        proposals: { proposed: 4, approved: 2, rejected: 1, enacted: 1 },
        protected_namespace: { total_checks: 3, allowed: 2, rejected: 1 },
        manifest_admission: {
          total_checks: 5,
          allowed: 3,
          missing_manifest: 1,
          non_validator_authority: 0,
          quorum_rejected: 1,
          protected_namespace_rejected: 0,
          runtime_hook_rejected: 0,
        },
        manifest_quorum: { total_checks: 2, satisfied: 1, rejected: 1 },
        recent_manifest_activations: [
          {
            namespace: "finance",
            contract_id: "alpha",
            code_hash_hex: "deadbeef",
            abi_hash_hex: null,
            height: 10,
            activated_at_ms: 1700,
          },
        ],
      },
      lane_commitments: [
        {
          block_height: 12,
          lane_id: 7,
          tx_count: 2,
          total_chunks: 3,
          rbc_bytes_total: 256,
          teu_total: 64,
          block_hash: "feedface",
        },
      ],
      dataspace_commitments: [
        {
          block_height: 12,
          lane_id: 7,
          dataspace_id: 9,
          tx_count: 1,
          total_chunks: 1,
          rbc_bytes_total: 128,
          teu_total: 16,
          block_hash: "facedead",
        },
      ],
      lane_governance: [
        {
          lane_id: 7,
          alias: "archive",
          dataspace_id: 9,
          visibility: "public",
          storage_profile: "full_replica",
          governance: null,
          manifest_required: true,
          manifest_ready: false,
          manifest_path: null,
          validator_ids: ["alice@test"],
          quorum: null,
          protected_namespaces: ["finance"],
        runtime_upgrade: {
          allow: true,
          require_metadata: false,
          metadata_key: null,
          allowed_ids: [],
        },
        privacy_commitments: [
          {
            id: 2,
            scheme: "merkle",
            merkle: { root: "0xabc123", max_depth: 8 },
          },
        ],
      },
    ],
      lane_governance_sealed_total: 2,
      lane_governance_sealed_aliases: ["archive", "payments"],
    },
    {
      peers: 5,
      queue_size: 1,
      commit_time_ms: 250,
      da_reschedule_total: 9,
      txs_approved: 103,
      txs_rejected: 4,
      view_changes: 2,
      governance: {
        proposals: { proposed: 5, approved: 3, rejected: 1, enacted: 1 },
        protected_namespace: { total_checks: 3, allowed: 2, rejected: 1 },
        manifest_admission: {
          total_checks: 6,
          allowed: 4,
          missing_manifest: 1,
          non_validator_authority: 0,
          quorum_rejected: 1,
          protected_namespace_rejected: 0,
          runtime_hook_rejected: 1,
        },
        manifest_quorum: { total_checks: 3, satisfied: 2, rejected: 1 },
        recent_manifest_activations: [
          {
            namespace: "finance",
            contract_id: "alpha",
            code_hash_hex: "deadbeef",
            abi_hash_hex: "b16b00b5",
            height: 11,
            activated_at_ms: 1900,
          },
        ],
      },
      lane_commitments: [
        {
          block_height: 13,
          lane_id: 8,
          tx_count: 1,
          total_chunks: 2,
          rbc_bytes_total: 200,
          teu_total: 48,
          block_hash: "cafebeef",
        },
      ],
      dataspace_commitments: [
        {
          block_height: 13,
          lane_id: 8,
          dataspace_id: 4,
          tx_count: 1,
          total_chunks: 1,
          rbc_bytes_total: 96,
          teu_total: 24,
          block_hash: "feedbead",
        },
      ],
      lane_governance: [
        {
          lane_id: 8,
          alias: "payments",
          dataspace_id: 4,
          visibility: "public",
          storage_profile: "full_replica",
          governance: "parliament",
          manifest_required: true,
          manifest_ready: true,
          manifest_path: "/etc/iroha/lanes/payments.json",
          validator_ids: ["bob@test", "carol@test"],
          quorum: 2,
          protected_namespaces: ["treasury"],
        runtime_upgrade: {
          allow: true,
          require_metadata: true,
          metadata_key: "upgrade_id",
          allowed_ids: ["payments-upgrade"],
        },
        privacy_commitments: [
          {
            id: 3,
            scheme: "snark",
            snark: {
              circuit_id: 7,
              verifying_key_digest: "0xaaaa",
              statement_hash: "0xbbbb",
              proof_hash: "0xcccc",
            },
          },
        ],
      },
    ],
      lane_governance_sealed_total: 0,
      lane_governance_sealed_aliases: [],
    },
  ];
  let callCount = 0;
  const fetchImpl = async (url, init = {}) => {
    assert.equal(url, `${BASE_URL}/v1/status`);
    assert.equal(init.method, "GET");
    assert.equal(init.headers.Accept, "application/json");
    const payload = payloads[callCount] ?? payloads[payloads.length - 1];
    callCount += 1;
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const first = await client.getStatusSnapshot();
  assert.equal(first.status.peers, 5);
  assert.equal(first.status.queue_size, 3);
  assert.equal(first.status.da_reschedule_total, 7);
  assert.equal(first.metrics.commit_latency_ms, 420);
  assert.equal(first.metrics.queue_delta, 0);
  assert.equal(first.metrics.tx_approved_delta, 0);
  assert.equal(first.metrics.has_activity, false);
  assert.equal(first.status.raw.commit_time_ms, 420);
  assert.ok(first.status.governance);
  assert.equal(first.status.governance?.manifest_admission.runtime_hook_rejected, 0);
  assert.deepEqual(first.status.lane_commitments, [
    {
      block_height: 12,
      lane_id: 7,
      tx_count: 2,
      total_chunks: 3,
      rbc_bytes_total: 256,
      teu_total: 64,
      block_hash: "feedface",
    },
  ]);
  assert.deepEqual(first.status.dataspace_commitments, [
    {
      block_height: 12,
      lane_id: 7,
      dataspace_id: 9,
      tx_count: 1,
      total_chunks: 1,
      rbc_bytes_total: 128,
      teu_total: 16,
      block_hash: "facedead",
    },
  ]);
  assert.deepEqual(first.status.lane_governance, [
    {
      lane_id: 7,
      alias: "archive",
      dataspace_id: 9,
      visibility: "public",
      storage_profile: "full_replica",
      governance: null,
      manifest_required: true,
      manifest_ready: false,
      manifest_path: null,
      validator_ids: ["alice@test"],
      quorum: null,
      protected_namespaces: ["finance"],
      runtime_upgrade: {
        allow: true,
        require_metadata: false,
        metadata_key: null,
        allowed_ids: [],
      },
      privacy_commitments: [
        {
          id: 2,
          scheme: "merkle",
          merkle: { root: "0xabc123", max_depth: 8 },
          snark: null,
        },
      ],
    },
  ]);
  assert.equal(first.status.lane_governance_sealed_total, 2);
  assert.deepEqual(first.status.lane_governance_sealed_aliases, ["archive", "payments"]);
  const activation = first.status.governance?.recent_manifest_activations[0];
  assert.equal(activation?.namespace, "finance");
  assert.equal(activation?.abi_hash_hex, null);

  const second = await client.getStatusSnapshot();
  assert.equal(second.status.queue_size, 1);
  assert.equal(second.metrics.queue_delta, -2);
  assert.equal(second.metrics.da_reschedule_delta, 2);
  assert.equal(second.metrics.tx_approved_delta, 3);
  assert.equal(second.metrics.tx_rejected_delta, 2);
  assert.equal(second.metrics.view_change_delta, 1);
  assert.equal(second.metrics.has_activity, true);
  assert.equal(second.status.governance?.manifest_admission.runtime_hook_rejected, 1);
  assert.equal(second.status.lane_governance_sealed_total, 0);
  assert.deepEqual(second.status.lane_governance_sealed_aliases, []);
  const secondActivation = second.status.governance?.recent_manifest_activations[0];
  assert.equal(secondActivation?.abi_hash_hex, "b16b00b5");
  assert.equal(callCount, 2);
});

test("getStatusSnapshot rejects non-integer counters", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        peers: 1.5,
        queue_size: 0,
        commit_time_ms: 1,
        da_reschedule_total: 0,
        txs_approved: 0,
        txs_rejected: 0,
        view_changes: 0,
        governance: null,
        lane_commitments: [],
        dataspace_commitments: [],
        lane_governance: [],
        lane_governance_sealed_total: 0,
        lane_governance_sealed_aliases: [],
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getStatusSnapshot(),
    (error) => {
      assert(error instanceof RangeError);
      assert.match(error.message, /status\.peers/);
      return true;
    },
  );
});

test("getStatusSnapshot rejects non-integer lane commitment values", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        peers: 1,
        queue_size: 0,
        commit_time_ms: 1,
        da_reschedule_total: 0,
        txs_approved: 0,
        txs_rejected: 0,
        view_changes: 0,
        governance: null,
        lane_commitments: [
          {
            block_height: 1,
            lane_id: 2,
            tx_count: 1.5,
            total_chunks: 0,
            rbc_bytes_total: 0,
            teu_total: 0,
            block_hash: "deadbeef",
          },
        ],
        dataspace_commitments: [],
        lane_governance: [],
        lane_governance_sealed_total: 0,
        lane_governance_sealed_aliases: [],
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getStatusSnapshot(),
    (error) => {
      assert(error instanceof RangeError);
      assert.match(error.message, /status\.lane_commitments\[0\]\.tx_count/);
      return true;
    },
  );
});

test("getStatusSnapshot forwards AbortSignal", async () => {
  const controller = new AbortController();
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/status`);
    assert.equal(init.headers.Accept, "application/json");
    assert.strictEqual(init.signal, controller.signal);
    return createResponse({
      status: 200,
      jsonData: { peers: 1, queue_size: 0, commit_time_ms: 1, da_reschedule_total: 0 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const snapshot = await client.getStatusSnapshot({ signal: controller.signal });
  assert.equal(snapshot.status.peers, 1);
});

test("getNetworkTimeNow normalizes timestamps", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        now: 1_000_000,
        offset_ms: -12,
        confidence_ms: 25,
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getNetworkTimeNow();
  assert.deepEqual(result, {
    timestampMs: 1_000_000,
    offsetMs: -12,
    confidenceMs: 25,
  });
});

test("getNetworkTimeNow rejects non-integer offsets", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        now: 1_000_000,
        offset_ms: 1.5,
        confidence_ms: 25,
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getNetworkTimeNow(),
    (error) => {
      assert(error instanceof RangeError);
      assert.match(error.message, /time now response\.offset_ms/);
      return true;
    },
  );
});

test("getNetworkTimeStatus normalizes diagnostics payload", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        peers: 2,
        samples: [
          {
            peer: "peer-a",
            last_offset_ms: -4,
            last_rtt_ms: 7,
            count: 11,
          },
        ],
        rtt: {
          buckets: [
            { le: 5, count: 2 },
            { le: 10, count: 3 },
          ],
          sum_ms: 42,
          count: 5,
        },
        note: "NTS running",
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getNetworkTimeStatus();
  assert.deepEqual(result, {
    peers: 2,
    samples: [{ peer: "peer-a", lastOffsetMs: -4, lastRttMs: 7, count: 11 }],
    rtt: {
      buckets: [
        { le: 5, count: 2 },
        { le: 10, count: 3 },
      ],
      sumMs: 42,
      count: 5,
    },
    note: "NTS running",
  });
});

test("getNetworkTimeStatus rejects malformed samples", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: { peers: 1, samples: null, rtt: { buckets: [], sum_ms: 0, count: 0 } },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getNetworkTimeStatus(),
    /time status response\.samples must be an array/,
  );
});

test("getNetworkTimeStatus rejects unsupported option fields", async () => {
  const fetchImpl = async () => {
    throw new Error("unexpected fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getNetworkTimeStatus({ signal: new AbortController().signal, extra: "nope" }),
    /getNetworkTimeStatus options contains unsupported fields: extra/,
  );
});

test("getNetworkTimeNow rejects non-object options", async () => {
  const fetchImpl = async () => {
    throw new Error("unexpected fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getNetworkTimeNow("oops"),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(error.message, /getNetworkTimeNow options must be an object/);
      return true;
    },
  );
});

test("getNetworkTimeNow rejects unsupported option fields", async () => {
  const fetchImpl = async () => {
    throw new Error("unexpected fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getNetworkTimeNow({ signal: new AbortController().signal, extra: true }),
    /getNetworkTimeNow options contains unsupported fields: extra/,
  );
});

test("getNodeCapabilities normalizes runtime advert", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        abi_version: 1,
        data_model_version: 1,
        crypto: {
          sm: {
            enabled: true,
            default_hash: "sm3",
            allowed_signing: ["sm2"],
            sm2_distid_default: "3132333435363738",
            openssl_preview: false,
            acceleration: {
              scalar: true,
              neon_sm3: true,
              neon_sm4: false,
              policy: "scalar",
            },
          },
          curves: {
            registry_version: 1,
            allowed_curve_ids: [1, 15],
            allowed_curve_bitmap: [32770],
          },
        },
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getNodeCapabilities();
  assert.deepEqual(result, {
    abiVersion: 1,
    dataModelVersion: 1,
    crypto: {
      sm: {
        enabled: true,
        defaultHash: "sm3",
        allowedSigning: ["sm2"],
        sm2DistIdDefault: "3132333435363738",
        opensslPreview: false,
        acceleration: {
          scalar: true,
          neonSm3: true,
          neonSm4: false,
          policy: "scalar",
        },
      },
      curves: {
        registryVersion: 1,
        allowedCurveIds: [1, 15],
        allowedCurveBitmap: [32770],
      },
    },
  });
});

test("getNodeCapabilities rejects non-integer ABI version", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        abi_version: 1.5,
        data_model_version: 1,
        crypto: {
          sm: {
            enabled: true,
            default_hash: "sm3",
            allowed_signing: ["sm2"],
            sm2_distid_default: "3132333435363738",
            openssl_preview: false,
            acceleration: {
              scalar: true,
              neon_sm3: true,
              neon_sm4: false,
              policy: "scalar",
            },
          },
          curves: {
            registry_version: 1,
            allowed_curve_ids: [1, 15],
            allowed_curve_bitmap: [32770],
          },
        },
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getNodeCapabilities(),
    (error) => {
      assert.match(error.message, /abi_version/);
      return true;
    },
  );
});

test("getNodeCapabilities rejects unsupported option fields", async () => {
  const fetchImpl = async () => {
    throw new Error("unexpected fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getNodeCapabilities({ signal: new AbortController().signal, extra: "nope" }),
    /getNodeCapabilities options contains unsupported fields: extra/,
  );
});

test("getRuntimeAbiActive normalizes ABI version", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: { abi_version: 1 },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getRuntimeAbiActive();
  assert.deepEqual(result, {
    abiVersion: 1,
  });
});

test("getRuntimeAbiActive rejects unsupported option fields", async () => {
  const fetchImpl = async () => {
    throw new Error("unexpected fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getRuntimeAbiActive({ signal: new AbortController().signal, extra: "nope" }),
    /getRuntimeAbiActive options contains unsupported fields: extra/,
  );
});

test("getRuntimeAbiHash enforces hex payload", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: { policy: "V1", abi_hash_hex: "aabb".repeat(16) },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getRuntimeAbiHash();
  assert.equal(result.policy, "V1");
  assert.equal(result.abiHashHex, "aabb".repeat(16));
});

test("getRuntimeAbiHash enforces AbortSignal option type", async () => {
  const fetchImpl = async () => {
    throw new Error("unexpected fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () =>
      client.getRuntimeAbiHash({
        // @ts-expect-error runtime validation should reject incorrect signal type
        signal: {},
      }),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(error.message, /getRuntimeAbiHash options\.signal must be an AbortSignal/);
      return true;
    },
  );
});

test("getRuntimeMetrics normalizes counters", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        abi_version: 1,
        upgrade_events_total: { proposed: 3, activated: 1, canceled: 1 },
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getRuntimeMetrics();
  assert.deepEqual(result, {
    abiVersion: 1,
    upgradeEventsTotal: { proposed: 3, activated: 1, canceled: 1 },
  });
});

test("listRuntimeUpgrades normalizes manifest and status payloads", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        items: [
          {
            id_hex: "aa".repeat(32),
            record: {
              manifest: {
                name: "ABI v1 refresh",
                description: "scheduled rollout",
                abi_version: 1,
                abi_hash: "11".repeat(32),
                added_syscalls: [],
                added_pointer_types: [],
                start_height: 10,
                end_height: 20,
              },
              status: { ActivatedAt: 12 },
              proposer: FIXTURE_ALICE_ID,
              created_height: 8,
            },
          },
          {
            id_hex: "bb".repeat(32),
            record: {
              manifest: {
                name: "ABI v1 maintenance",
                description: "next window",
                abi_version: 1,
                abi_hash: "22".repeat(32),
                added_syscalls: [],
                added_pointer_types: [],
                start_height: 30,
                end_height: 40,
              },
              status: { Proposed: null },
              proposer: FIXTURE_BOB_ID,
              created_height: 25,
            },
          },
        ],
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const upgrades = await client.listRuntimeUpgrades();
  assert.equal(upgrades.length, 2);
  assert.deepEqual(upgrades[0], {
    idHex: "aa".repeat(32),
    record: {
      manifest: {
        name: "ABI v1 refresh",
        description: "scheduled rollout",
        abiVersion: 1,
        abiHashHex: "11".repeat(32),
        addedSyscalls: [],
        addedPointerTypes: [],
        startHeight: 10,
        endHeight: 20,
      },
      status: { kind: "ActivatedAt", activatedHeight: 12 },
      proposer: FIXTURE_ALICE_ID,
      createdHeight: 8,
    },
  });
  assert.deepEqual(upgrades[1].record.status, { kind: "Proposed" });
});

test("proposeRuntimeUpgrade posts manifest and normalizes response", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: {
        ok: true,
        tx_instructions: [{ wire_id: "ProposeRuntimeUpgrade", payload_hex: "aa".repeat(32) }],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const manifest = {
    name: "ABI v1 maintenance",
    description: "roll out refreshed binaries",
    abiVersion: 1,
    abiHash: "11".repeat(32),
    startHeight: 100,
    endHeight: 200,
    addedSyscalls: [],
    addedPointerTypes: [],
  };
  const result = await client.proposeRuntimeUpgrade(manifest);
  assert.equal(captured.url, `${BASE_URL}/v1/runtime/upgrades/propose`);
  assert.equal(captured.init.method, "POST");
  const body = JSON.parse(captured.init.body);
  assert.equal(body.name, "ABI v1 maintenance");
  assert.equal(body.abi_version, 1);
  assert.deepEqual(body.added_syscalls, []);
  assert.deepEqual(result, {
    ok: true,
    tx_instructions: [{ wire_id: "ProposeRuntimeUpgrade", payload_hex: "aa".repeat(32) }],
  });
});

test("listRuntimeUpgrades rejects non-v1 manifest ABI", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        items: [
          {
            id_hex: "aa".repeat(32),
            record: {
              manifest: {
                name: "invalid",
                description: "bad abi",
                abi_version: 2,
                abi_hash: "11".repeat(32),
                added_syscalls: [],
                added_pointer_types: [],
                start_height: 10,
                end_height: 20,
              },
              status: { Proposed: null },
              proposer: FIXTURE_ALICE_ID,
              created_height: 8,
            },
          },
        ],
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listRuntimeUpgrades(),
    /abi_version must be 1 in the first release/,
  );
});

test("proposeRuntimeUpgrade rejects added syscall deltas in the first release", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () =>
      client.proposeRuntimeUpgrade({
        name: "invalid",
        description: "bad delta",
        abiVersion: 1,
        abiHash: "11".repeat(32),
        startHeight: 100,
        endHeight: 200,
        addedSyscalls: [600],
      }),
    /added_syscalls must be empty in the first release/,
  );
});

test("activateRuntimeUpgrade posts id and normalizes response", async () => {
  let calledUrl;
  const fetchImpl = async (url) => {
    calledUrl = url;
    return createResponse({
      status: 200,
      jsonData: { ok: true, tx_instructions: [{ wire_id: "ActivateRuntimeUpgrade" }] },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const response = await client.activateRuntimeUpgrade("0x" + "ff".repeat(32));
  assert.equal(
    calledUrl,
    `${BASE_URL}/v1/runtime/upgrades/activate/0x${"ff".repeat(32)}`,
  );
  assert.deepEqual(response, {
    ok: true,
    tx_instructions: [{ wire_id: "ActivateRuntimeUpgrade" }],
  });
});

test("cancelRuntimeUpgrade posts id and normalizes response", async () => {
  let calledUrl;
  const fetchImpl = async (url) => {
    calledUrl = url;
    return createResponse({
      status: 200,
      jsonData: { ok: true, tx_instructions: [{ wire_id: "CancelRuntimeUpgrade" }] },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const response = await client.cancelRuntimeUpgrade("aa".repeat(32));
  assert.equal(
    calledUrl,
    `${BASE_URL}/v1/runtime/upgrades/cancel/0x${"aa".repeat(32)}`,
  );
  assert.deepEqual(response, {
    ok: true,
    tx_instructions: [{ wire_id: "CancelRuntimeUpgrade" }],
  });
});

test("runtime upgrade wrappers reject unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  for (const [label, invoke, key] of [
    ["getRuntimeAbiHash", () => client.getRuntimeAbiHash({ extra: true }), "extra"],
    ["getRuntimeMetrics", () => client.getRuntimeMetrics({ note: "nope" }), "note"],
    ["listRuntimeUpgrades", () => client.listRuntimeUpgrades({ surprise: 1 }), "surprise"],
    [
      "activateRuntimeUpgrade",
      () => client.activateRuntimeUpgrade("aa".repeat(32), { junk: true }),
      "junk",
    ],
    [
      "proposeRuntimeUpgrade",
      () =>
        client.proposeRuntimeUpgrade(
          {
            name: "upgrade",
            description: "test",
            abi_version: 1,
            abi_hash: "aa".repeat(32),
            start_height: 0,
            end_height: 1,
          },
          { note: "skip" },
        ),
      "note",
    ],
    [
      "cancelRuntimeUpgrade",
      () => client.cancelRuntimeUpgrade("aa".repeat(32), { unsupported: true }),
      "unsupported",
    ],
  ]) {
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(invoke, (error) => {
      assert(error instanceof TypeError);
      assert.match(
        error.message,
        new RegExp(`${label} options contains unsupported fields: ${key}`),
      );
      return true;
    });
  }
});

test("getGovernanceProposalTyped parses DeployContract variant", async () => {
  const fetchImpl = async (url) => {
    assert.equal(url, `${BASE_URL}/v1/gov/proposals/prop-1`);
    return createResponse({
      status: 200,
      jsonData: cloneFixture(toriiFixtures.governance.proposalDeployContract),
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getGovernanceProposalTyped("prop-1");
  assert.equal(result.found, true);
  assert.ok(result.proposal);
  assert.equal(result.proposal?.status, "Approved");
  assert.equal(result.proposal?.kind.variant, "DeployContract");
  assert.equal(result.proposal?.kind.deploy_contract?.contract_id, "router");

  const notFoundClient = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 404 }),
  });
  const missing = await notFoundClient.getGovernanceProposal("missing");
  assert.equal(missing, null);

  const missingTyped = await notFoundClient.getGovernanceProposalTyped("missing");
  assert.deepEqual(missingTyped, { found: false, proposal: null });
});

test("getGovernanceProposal forwards AbortSignal option", async () => {
  const controller = new AbortController();
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: cloneFixture(toriiFixtures.governance.proposalDeployContract),
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.getGovernanceProposal("prop-42", { signal: controller.signal });
  assert.equal(captured.url, `${BASE_URL}/v1/gov/proposals/prop-42`);
  assert.ok(captured.init.signal instanceof AbortSignal);
});

test("getGovernanceProposal enforces options shape", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 404 }) });
  await assert.rejects(
    () => client.getGovernanceProposal("prop-1", "oops"),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(error.message, /getGovernanceProposal options must be an object/);
      return true;
    },
  );
  await assert.rejects(
    () =>
      client.getGovernanceProposal("prop-1", {
        // @ts-expect-error invalid signal for runtime validation test
        signal: "nope",
      }),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(error.message, /options\.signal must be an AbortSignal/);
      return true;
    },
  );
});

test("getGovernanceProposal rejects empty payloads", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: null,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getGovernanceProposal("prop-1"),
    /governance proposal endpoint returned no payload/,
  );
});

test("getGovernanceReferendum rejects empty payloads", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: null,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getGovernanceReferendum("ref-1"),
    /governance referendum endpoint returned no payload/,
  );
});

test("governance query wrappers reject unknown option fields", async () => {
  const fetchImpl = async () => {
    throw new Error("fetch should not be called when options are invalid");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const badOptions = { signal: new AbortController().signal, extra: "nope" };
  const cases = [
    ["getGovernanceProposal", () => client.getGovernanceProposal("prop-1", badOptions)],
    ["getGovernanceReferendum", () =>
      client.getGovernanceReferendum("ref-1", badOptions)],
    ["getGovernanceTally", () => client.getGovernanceTally("ref-1", badOptions)],
    ["getGovernanceLocks", () => client.getGovernanceLocks("ref-1", badOptions)],
    ["getGovernanceUnlockStats", () => client.getGovernanceUnlockStats(badOptions)],
  ];
  for (const [label, invoke] of cases) {
    await assert.rejects(invoke, (error) => {
      assert(error instanceof TypeError);
      assert.match(
        error.message,
        new RegExp(`${label} options contains unsupported fields: extra`),
      );
      return true;
    });
  }
});

test("getGovernanceLocks rejects empty payloads", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: null,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getGovernanceLocks("ref-1"),
    /governance locks endpoint returned no payload/,
  );
});

test("getGovernanceUnlockStats rejects empty payloads", async () => {
  const fetchImpl = async () => createResponse({ status: 200 });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getGovernanceUnlockStats(),
    /governance unlock stats endpoint returned no payload/,
  );
});

test("getGovernanceReferendumTyped tolerates missing referendum payloads", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: cloneFixture(toriiFixtures.governance.referendumMissing),
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getGovernanceReferendumTyped("ref-1");
  assert.equal(result.found, false);
  assert.equal(result.referendum, null);
});

test("getGovernanceReferendum treats 404 as not found", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 404 }),
  });
  const raw = await client.getGovernanceReferendum("missing-ref");
  assert.equal(raw, null);
  const typed = await client.getGovernanceReferendumTyped("missing-ref");
  assert.equal(typed.found, false);
  assert.equal(typed.referendum, null);
});

test("getGovernanceLocksTyped parses lock records and synthesizes not-found result on 404", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: cloneFixture(toriiFixtures.governance.locks),
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getGovernanceLocksTyped("ref-1");
  assert.equal(result.found, true);
  assert.equal(Object.keys(result.locks).length, 1);
  const [firstLock] = Object.values(result.locks);
  assert.ok(firstLock);
  assert.equal(firstLock.duration_blocks, 5);

  const missingClient = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 404 }),
  });
  const raw = await missingClient.getGovernanceLocks("ref-2");
  assert.equal(raw, null);
  const missing = await missingClient.getGovernanceLocksTyped("  ref-2  ");
  assert.deepEqual(missing, {
    found: false,
    locks: {},
    referendum_id: "ref-2",
  });
});

test("getGovernanceUnlockStatsTyped normalizes numeric fields", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: cloneFixture(toriiFixtures.governance.unlockStats),
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const stats = await client.getGovernanceUnlockStatsTyped();
  assert.equal(stats.height_current, 100);
  assert.equal(stats.expired_locks_now, 2);
  assert.equal(stats.referenda_with_expired, 1);
  assert.equal(stats.last_sweep_height, 95);
});

test("getGovernanceTallyTyped parses referendum votes and trims selector", async () => {
  let capturedUrl;
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: cloneFixture(toriiFixtures.governance.tally),
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const tally = await client.getGovernanceTallyTyped("  ref-1  ");
  assert.equal(capturedUrl, `${BASE_URL}/v1/gov/tally/ref-1`);
  assert.deepEqual(tally, {
    found: true,
    referendum_id: "ref-1",
    tally: {
      referendum_id: "ref-1",
      approve: 7,
      reject: 3,
      abstain: 1,
    },
  });
});

test("getGovernanceTallyTyped rejects empty tally payloads", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: null,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getGovernanceTallyTyped("ref-1"),
    /governance tally endpoint returned no payload/,
  );
});

test("getGovernanceTallyTyped returns empty result for missing referendum", async () => {
  const missingClient = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 404 }),
  });
  const raw = await missingClient.getGovernanceTally("  ref-2  ");
  assert.equal(raw, null);
  const missing = await missingClient.getGovernanceTallyTyped("  ref-2  ");
  assert.deepEqual(missing, {
    found: false,
    referendum_id: "ref-2",
    tally: null,
  });
});

test("governanceFinalizeReferendum and governanceEnactProposal normalize payloads", async () => {
  const captures = [];
  const fetchImpl = async (url, init) => {
    captures.push({ url, init });
    if (url.endsWith("/finalize")) {
      return createResponse({ status: 204 });
    }
    return createResponse({
      status: 200,
      jsonData: cloneFixture(toriiFixtures.governance.draftResponse),
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const finalizeProposal = `0x${"ab".repeat(32)}`;
  const enactProposal = `0x${"cd".repeat(32)}`;
  const preimageHash = `0x${"ef".repeat(32)}`;
  const finalizeHex = "ab".repeat(32);
  const enactHex = "cd".repeat(32);
  const preimageHex = "ef".repeat(32);
  const finalizeResult = await client.governanceFinalizeReferendum({
    referendumId: " ref-3 ",
    proposalId: finalizeProposal,
  });
  assert.equal(finalizeResult, null);
  const enact = await client.governanceEnactProposal({
    proposalId: enactProposal.toUpperCase(),
    preimageHash,
    window: { lower: 10, upper: 42 },
  });
  assert.deepEqual(enact, {
    ok: true,
    proposal_id: enactHex,
    tx_instructions: [
      { wire_id: "FinalizeReferendum", payload_hex: "aaaaaaaa" },
      { wire_id: "PersistCouncilForEpoch" },
    ],
  });
  assert.equal(captures.length, 2);
  assert.equal(captures[0].url, `${BASE_URL}/v1/gov/finalize`);
  assert.equal(captures[1].url, `${BASE_URL}/v1/gov/enact`);
  const finalizeBody = JSON.parse(String(captures[0].init.body));
  assert.deepEqual(finalizeBody, {
    referendum_id: "ref-3",
    proposal_id: finalizeHex,
  });
  const enactBody = JSON.parse(String(captures[1].init.body));
  assert.deepEqual(enactBody, {
    proposal_id: enactHex,
    preimage_hash: preimageHex,
    window: { lower: 10, upper: 42 },
  });
  for (const capture of captures) {
    assert.equal(capture.init.method, "POST");
    assert.equal(capture.init.headers["Content-Type"], "application/json");
  }
});

test("typed governance finalize/enact helpers always return drafts", async () => {
  const captures = [];
  const fetchImpl = async (url, init) => {
    captures.push({ url, init });
    if (url.endsWith("/finalize")) {
      return createResponse({ status: 204 });
    }
    return createResponse({
      status: 200,
      jsonData: cloneFixture(toriiFixtures.governance.draftResponse),
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const fallback = await client.governanceFinalizeReferendumTyped({
    referendumId: "ref-204",
    proposalId: `0x${"01".repeat(32)}`,
  });
  assert.deepEqual(fallback, {
    ok: true,
    proposal_id: null,
    tx_instructions: [],
  });
  const typedEnact = await client.governanceEnactProposalTyped({
    proposalId: `0x${"02".repeat(32)}`,
    preimageHash: `0x${"03".repeat(32)}`,
  });
  assert.deepEqual(typedEnact, {
    ok: true,
    proposal_id: "cd".repeat(32),
    tx_instructions: [
      { wire_id: "FinalizeReferendum", payload_hex: "aaaaaaaa" },
      { wire_id: "PersistCouncilForEpoch" },
    ],
  });
  assert.equal(captures.length, 2);
  assert.equal(captures[0].url, `${BASE_URL}/v1/gov/finalize`);
  assert.equal(captures[1].url, `${BASE_URL}/v1/gov/enact`);
});

test("governanceProposeDeployContract normalizes payloads", async () => {
  let capturedBody;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url, init) => {
      capturedBody = JSON.parse(init.body);
      return createResponse({
        status: 200,
        jsonData: cloneFixture(toriiFixtures.governance.deployContractDraft),
        headers: { "content-type": "application/json" },
      });
    },
  });
  const result = await client.governanceProposeDeployContract({
    namespace: " apps ",
    contractId: "calc.v1",
    codeHash: `0x${"1a".repeat(32)}`,
    abiHash: Buffer.alloc(32, 0xbb),
    abiVersion: "1",
    window: { lower: 10, upper: 42 },
    mode: "plain",
    limits: { maxTx: 5 },
  });
  assert.equal(capturedBody.namespace, "apps");
  assert.equal(capturedBody.contract_id, "calc.v1");
  assert.equal(capturedBody.code_hash, "1a".repeat(32));
  assert.equal(capturedBody.abi_hash, "bb".repeat(32));
  assert.equal(capturedBody.mode, "Plain");
  assert.deepEqual(capturedBody.window, { lower: 10, upper: 42 });
  assert.deepEqual(capturedBody.limits, { maxTx: 5 });
  assert.equal(result.proposal_id, "cd".repeat(32));
});

test("governanceProposeDeployContract accepts byte-array hashes", async () => {
  let capturedBody;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (_url, init) => {
      capturedBody = JSON.parse(init.body);
      return createResponse({
        status: 200,
        jsonData: cloneFixture(toriiFixtures.governance.deployContractDraft),
        headers: { "content-type": "application/json" },
      });
    },
  });

  await client.governanceProposeDeployContract({
    namespace: "apps",
    contractId: "calc.v1",
    codeHash: Array.from(Buffer.alloc(32, 0x1a)),
    abiHash: Array.from(Buffer.alloc(32, 0xbb)),
  });

  assert.equal(capturedBody.code_hash, "1a".repeat(32));
  assert.equal(capturedBody.abi_hash, "bb".repeat(32));
});

test("governanceProposeDeployContract rejects non-byte hash arrays", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: cloneFixture(toriiFixtures.governance.deployContractDraft),
        headers: { "content-type": "application/json" },
      }),
  });

  await assert.rejects(
    () =>
      client.governanceProposeDeployContract({
        namespace: "apps",
        contractId: "calc.v1",
        codeHash: [256],
        abiHash: Array.from(Buffer.alloc(32, 0xbb)),
      }),
    (error) =>
      error?.name === "ValidationError" &&
      /governanceProposeDeployContract\.code_hash\[0\]/i.test(error.message),
  );
});

test("governanceSubmitPlainBallot normalizes amount and direction", async () => {
  let capturedBody;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url, init) => {
      capturedBody = JSON.parse(init.body);
      return createResponse({
        status: 200,
        jsonData: cloneFixture(toriiFixtures.governance.plainBallotResponse),
        headers: { "content-type": "application/json" },
      });
    },
  });
  const ballot = await client.governanceSubmitPlainBallot({
    authority: FIXTURE_ALICE_ID,
    chainId: "chain-0",
    referendumId: "ref-plain",
    owner: FIXTURE_ALICE_ID,
    amount: 500n,
    durationBlocks: "600",
    direction: "nay",
  });
  assert.equal(capturedBody.amount, "500");
  assert.equal(capturedBody.duration_blocks, 600);
  assert.equal(capturedBody.direction, "Nay");
  assert.equal(ballot.accepted, true);
});

test("governanceSubmitPlainBallot accepts decimal Numeric amounts", async () => {
  let capturedBody;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (_url, init) => {
      capturedBody = JSON.parse(init.body);
      return createResponse({
        status: 200,
        jsonData: cloneFixture(toriiFixtures.governance.plainBallotResponse),
        headers: { "content-type": "application/json" },
      });
    },
  });
  await client.governanceSubmitPlainBallot({
    authority: FIXTURE_ALICE_ID,
    chainId: "chain-0",
    referendumId: "ref-plain-decimal",
    owner: FIXTURE_ALICE_ID,
    amount: "12.500",
    durationBlocks: 1,
    direction: "aye",
  });
  assert.equal(capturedBody.amount, "12.500");
});

test("governanceSubmitPlainBallot forwards AbortSignal to fetch", async () => {
  let observedSignal;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url, init) => {
      observedSignal = init?.signal;
      return createResponse({
        status: 200,
        jsonData: cloneFixture(toriiFixtures.governance.plainBallotResponse),
        headers: { "content-type": "application/json" },
      });
    },
  });
  const controller = new AbortController();
  await client.governanceSubmitPlainBallot(
    {
      authority: FIXTURE_ALICE_ID,
      chainId: "chain-0",
      referendumId: "ref-plain",
      owner: FIXTURE_ALICE_ID,
      amount: "5000",
      durationBlocks: 1_000,
      direction: "Aye",
    },
    { signal: controller.signal },
  );
  assert.equal(observedSignal, controller.signal);
});

test("protected namespace helpers normalize payloads and support AbortSignal", async () => {
  const captures = [];
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url, init = {}) => {
      captures.push({ url, init });
      const payload = init.method === "POST"
        ? toriiFixtures.governance.protectedNamespacesApply
        : toriiFixtures.governance.protectedNamespacesGet;
      return createResponse({
        status: 200,
        jsonData: cloneFixture(payload),
        headers: { "content-type": "application/json" },
      });
    },
  });
  const controller = new AbortController();
  const applyResponse = await client.setProtectedNamespaces([" apps ", "system"], {
    signal: controller.signal,
  });
  assert.equal(captures[0].url, `${BASE_URL}/v1/gov/protected-namespaces`);
  assert.equal(captures[0].init.method, "POST");
  assert.equal(captures[0].init.signal, controller.signal);
  assert.deepEqual(JSON.parse(String(captures[0].init.body)), {
    namespaces: ["apps", "system"],
  });
  assert.equal(applyResponse.ok, true);
  assert.equal(applyResponse.applied, 2);

  const getResponse = await client.getProtectedNamespaces({ signal: controller.signal });
  assert.equal(captures[1].url, `${BASE_URL}/v1/gov/protected-namespaces`);
  assert.equal(captures[1].init.method, "GET");
  assert.equal(captures[1].init.signal, controller.signal);
  assert.equal(getResponse.found, true);
  assert.deepEqual(getResponse.namespaces, ["apps", "system"]);
});

test("protected namespace helpers validate options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () => client.setProtectedNamespaces(["apps"], 123),
    /setProtectedNamespaces options must be an object/,
  );
  await assert.rejects(
    () => client.setProtectedNamespaces(["apps"], { signal: "oops" }),
    /setProtectedNamespaces options\.signal must be an AbortSignal/,
  );
  await assert.rejects(
    () => client.getProtectedNamespaces(123),
    /getProtectedNamespaces options must be an object/,
  );
});

test("governance helpers reject unsupported option keys", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not be invoked for option validation");
    },
  });
  await assert.rejects(
    () => client.getGovernanceCouncilCurrent({ signal: undefined, extra: true }),
    /getGovernanceCouncilCurrent options contains unsupported fields: extra/,
  );
  const ballotPayload = {
    authority: FIXTURE_ALICE_ID,
    chainId: "chain-1",
    referendumId: "ref-1",
    owner: FIXTURE_ALICE_ID,
    amount: "10",
    durationBlocks: 1,
    direction: "Aye",
  };
  await assert.rejects(
    () => client.governanceSubmitPlainBallot(ballotPayload, { unexpected: 1 }),
    /governanceSubmitPlainBallot options contains unsupported fields: unexpected/,
  );
});

test("governanceProposeDeployContract rejects invalid signal options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not fetch when signal is invalid");
    },
  });
  await assert.rejects(
    () =>
      client.governanceProposeDeployContract(
        {
          namespace: "apps",
          contractId: "calc.v1",
          codeHash: "hash:DEMO",
          abiHash: Buffer.alloc(32, 0xaa),
          abiVersion: "1",
          window: { lower: 1, upper: 5 },
          mode: "Plain",
        },
        // @ts-expect-error: exercised to assert runtime validation.
        { signal: {} },
      ),
    /governanceProposeDeployContract options\.signal must be an AbortSignal/,
  );
});

test("governanceSubmitZk ballots encode proofs and hints", async () => {
  const calls = [];
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url, init) => {
      calls.push({ url, body: JSON.parse(init.body) });
      const payload = url.endsWith("/ballots/zk")
        ? toriiFixtures.governance.zkBallotAccepted
        : toriiFixtures.governance.zkBallotDeferred;
      return createResponse({
        status: 200,
        jsonData: cloneFixture(payload),
        headers: { "content-type": "application/json" },
      });
    },
  });
  const zkResult = await client.governanceSubmitZkBallot({
    authority: FIXTURE_BOB_ID,
    chainId: "chain-0",
    electionId: "ref-zk",
    proof: [1, 2, 3],
    public: {
      owner: SAMPLE_ACCOUNT_FORMS.i105,
      amount: "42",
      duration_blocks: 128,
      direction: "Aye",
      root_hint: `0x${"Aa".repeat(32)}`,
      nullifier: `blake2b32:${"BB".repeat(32)}`,
    },
  });
  assert.equal(calls[0].body.proof_b64, "AQID");
  assert.deepEqual(calls[0].body.public, {
    owner: SAMPLE_ACCOUNT_FORMS.i105,
    amount: "42",
    duration_blocks: 128,
    direction: "Aye",
    root_hint: "aa".repeat(32),
    nullifier: "bb".repeat(32),
  });
  assert.equal(zkResult.accepted, true);

  const zkV1Result = await client.governanceSubmitZkBallotV1({
    authority: FIXTURE_BOB_ID,
    chainId: "chain-0",
    electionId: "ref-zk",
    backend: "halo2/ipa",
    envelope: [4, 5],
    root_hint: `blake2b32:${"Ab".repeat(32)}`,
    owner: null,
    nullifier: Buffer.alloc(32, 0xff),
  });
  assert.equal(calls[1].body.envelope_b64, "BAU=");
  assert.equal(calls[1].body.root_hint, "ab".repeat(32));
  assert.equal(calls[1].body.nullifier, "ff".repeat(32));
  assert.equal(zkV1Result.accepted, false);
  assert.equal(zkV1Result.reason, "build transaction skeleton");

  const zkProofResult = await client.governanceSubmitZkBallotProofV1({
    authority: FIXTURE_BOB_ID,
    chainId: "chain-0",
    electionId: "ref-zk",
    ballot: {
      backend: "halo2/ipa",
      envelope_bytes: "AAE=",
      root_hint: `blake2b32:${"Cc".repeat(32)}`,
      nullifier: `0x${"DD".repeat(32)}`,
      owner: null,
      amount: null,
      duration_blocks: null,
      direction: null,
    },
  });
  assert.equal(calls[2].body.ballot.root_hint, "cc".repeat(32));
  assert.equal(calls[2].body.ballot.nullifier, "dd".repeat(32));
  assert.equal(zkProofResult.accepted, false);
});

test("governanceSubmitZk ballots reject partial lock hints", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () =>
      client.governanceSubmitZkBallot({
        authority: FIXTURE_BOB_ID,
        chainId: "chain-0",
        electionId: "ref-zk",
        proof: [1, 2, 3],
        public: { owner: SAMPLE_ACCOUNT_FORMS.i105, amount: "42" },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /owner, amount, and duration_blocks/i);
      return true;
    },
  );
});

test("governanceSubmitZk ballots reject invalid hex hints", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () =>
      client.governanceSubmitZkBallot({
        authority: FIXTURE_BOB_ID,
        chainId: "chain-0",
        electionId: "ref-zk",
        proof: [1, 2, 3],
        public: {
          owner: SAMPLE_ACCOUNT_FORMS.i105,
          amount: "42",
          duration_blocks: 128,
          root_hint: "not-hex",
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_HEX);
      assert.match(String(error?.message), /root_hint/i);
      return true;
    },
  );
});

test("governanceSubmitZk ballots reject deprecated public input keys", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () =>
      client.governanceSubmitZkBallot({
        authority: FIXTURE_BOB_ID,
        chainId: "chain-0",
        electionId: "ref-zk",
        proof: [1, 2, 3],
        public: {
          owner: SAMPLE_ACCOUNT_FORMS.i105,
          amount: "42",
          durationBlocks: 128,
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /durationBlocks/i);
      return true;
    },
  );
});

test("governanceSubmitZk ballots reject noncanonical owners", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () =>
      client.governanceSubmitZkBallot({
        authority: FIXTURE_BOB_ID,
        chainId: "chain-0",
        electionId: "ref-zk",
        proof: [1, 2, 3],
        public: {
          owner: SAMPLE_ACCOUNT_FORMS.nonCanonicalI105,
          amount: "42",
          duration_blocks: 128,
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_ACCOUNT_ID);
      assert.match(String(error?.message), /canonical (?:I105 )?account id/i);
      return true;
    },
  );
});

test("governanceSubmitZkBallotV1 rejects partial lock hints", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () =>
      client.governanceSubmitZkBallotV1({
        authority: FIXTURE_BOB_ID,
        chainId: "chain-0",
        electionId: "ref-zk",
        backend: "halo2/ipa",
        envelope: [4, 5],
        owner: SAMPLE_ACCOUNT_FORMS.i105,
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /owner, amount, and duration_blocks/i);
      return true;
    },
  );
});

test("governanceSubmitZkBallotV1 rejects noncanonical owner", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () =>
      client.governanceSubmitZkBallotV1({
        authority: FIXTURE_BOB_ID,
        chainId: "chain-0",
        electionId: "ref-zk",
        backend: "halo2/ipa",
        envelope: [4, 5],
        owner: SAMPLE_ACCOUNT_FORMS.nonCanonicalI105,
        amount: "42",
        duration_blocks: 128,
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_ACCOUNT_ID);
      assert.match(String(error?.message), /canonical (?:I105 )?account id/i);
      return true;
    },
  );
});

test("governanceSubmitZkBallotV1 rejects invalid hex hints", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () =>
      client.governanceSubmitZkBallotV1({
        authority: FIXTURE_BOB_ID,
        chainId: "chain-0",
        electionId: "ref-zk",
        backend: "halo2/ipa",
        envelope: [4, 5],
        root_hint: "not-hex",
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_HEX);
      assert.match(String(error?.message), /root_hint/i);
      return true;
    },
  );
});

test("governanceSubmitZkBallotProofV1 rejects partial lock hints", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () =>
      client.governanceSubmitZkBallotProofV1({
        authority: FIXTURE_BOB_ID,
        chainId: "chain-0",
        electionId: "ref-zk",
        ballot: { owner: SAMPLE_ACCOUNT_FORMS.i105 },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /owner, amount, and duration_blocks/i);
      return true;
    },
  );
});

test("governanceSubmitZkBallotProofV1 rejects noncanonical owner", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () =>
      client.governanceSubmitZkBallotProofV1({
        authority: FIXTURE_BOB_ID,
        chainId: "chain-0",
        electionId: "ref-zk",
        ballot: {
          owner: SAMPLE_ACCOUNT_FORMS.nonCanonicalI105,
          amount: "42",
          duration_blocks: 128,
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_ACCOUNT_ID);
      assert.match(String(error?.message), /canonical (?:I105 )?account id/i);
      return true;
    },
  );
});

test("getGovernanceCouncilCurrent normalizes roster payload", async () => {
  let callCount = 0;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      callCount += 1;
      const payload = cloneFixture(toriiFixtures.governance.councilCurrent);
      if (Array.isArray(payload.members) && payload.members.length >= 2) {
        payload.members[0].account_id = FIXTURE_ALICE_ID;
        payload.members[1].account_id = FIXTURE_BOB_ID;
      }
      if (Array.isArray(payload.alternates) && payload.alternates.length > 0) {
        payload.alternates[0].account_id = FIXTURE_CAROL_ID;
      }
      return createResponse({
        status: 200,
        jsonData: payload,
        headers: { "content-type": "application/json" },
      });
    },
  });
  const roster = await client.getGovernanceCouncilCurrent();
  assert.equal(callCount, 1);
  assert.equal(roster.epoch, 77);
  assert.deepEqual(roster.members, [
    { account_id: FIXTURE_ALICE_ID },
    { account_id: FIXTURE_BOB_ID },
  ]);
});

test("getGovernanceCouncilCurrent rejects non-object options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () => client.getGovernanceCouncilCurrent("bad-options"),
    /getGovernanceCouncilCurrent options must be an object/,
  );
});

test("governanceDeriveCouncilVrf encodes candidate payload", async () => {
  let captured;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (_url, init) => {
      captured = JSON.parse(init.body);
      const payload = cloneFixture(toriiFixtures.governance.councilDerive);
      if (Array.isArray(payload.members) && payload.members.length > 0) {
        payload.members[0].account_id = FIXTURE_VALIDATOR_TEST_ID;
      }
      return createResponse({
        status: 200,
        jsonData: payload,
        headers: { "content-type": "application/json" },
      });
    },
  });
  const payload = await client.governanceDeriveCouncilVrf({
    committeeSize: "3",
    epoch: "9",
    candidates: [
      {
        accountId: FIXTURE_VALIDATOR_TEST_ID,
        variant: "small",
        pk_b64: Buffer.alloc(48, 0xaa),
        proof_b64: Buffer.alloc(96, 0xbb),
      },
    ],
  });
  assert.equal(captured.committee_size, 3);
  assert.equal(captured.epoch, 9);
  assert.equal(captured.candidates.length, 1);
  assert.equal(captured.candidates[0].account_id, FIXTURE_VALIDATOR_TEST_ID);
  assert.equal(captured.candidates[0].variant, "Small");
  assert.match(captured.candidates[0].pk_b64, /^[A-Za-z0-9+/]+=*$/);
  assert.match(captured.candidates[0].proof_b64, /^[A-Za-z0-9+/]+=*$/);
  assert.equal(payload.members[0].account_id, FIXTURE_VALIDATOR_TEST_ID);
  assert.equal(payload.total_candidates, 1);
  assert.equal(payload.verified, 1);
  assert.equal(payload.derived_by, "Vrf");
});

test("governancePersistCouncil forwards credentials and validates pairing", async () => {
  let captured;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (_url, init) => {
      captured = JSON.parse(init.body);
      return createResponse({
        status: 200,
        jsonData: cloneFixture(toriiFixtures.governance.councilPersist),
        headers: { "content-type": "application/json" },
      });
    },
  });
  const response = await client.governancePersistCouncil({
    committeeSize: 1,
    candidates: [
      {
        accountId: FIXTURE_VALIDATOR_TEST_ID,
        variant: "Normal",
        pk_b64: Buffer.alloc(48).toString("base64"),
        proof_b64: Buffer.alloc(96).toString("base64"),
      },
    ],
    authority: FIXTURE_COUNCIL_TEST_ID,
    privateKey: "ed25519:deadbeef",
  });
  assert.equal(captured.authority, FIXTURE_COUNCIL_TEST_ID);
  assert.equal(captured.private_key, "ed25519:deadbeef");
  assert.equal(response.derived_by, "Vrf");
  await assert.rejects(
    () =>
      client.governancePersistCouncil({
        committeeSize: 1,
        candidates: [
          {
            accountId: FIXTURE_VALIDATOR_TEST_ID,
            variant: "Normal",
            pk_b64: Buffer.alloc(48).toString("base64"),
            proof_b64: Buffer.alloc(96).toString("base64"),
          },
        ],
        authority: FIXTURE_COUNCIL_TEST_ID,
      }),
    /requires both authority and privateKey/,
  );
});

test("getGovernanceCouncilAudit forwards query parameters", async () => {
  let capturedUrl;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url) => {
      capturedUrl = url;
      return createResponse({
        status: 200,
        jsonData: cloneFixture(toriiFixtures.governance.councilAudit),
        headers: { "content-type": "application/json" },
      });
    },
  });
  const audit = await client.getGovernanceCouncilAudit({ epoch: 10 });
  assert.equal(
    capturedUrl,
    `${BASE_URL}/v1/gov/council/audit?epoch=10`,
  );
  assert.equal(audit.epoch, 10);
  assert.equal(audit.members_count, 5);
  assert.equal(audit.candidate_count, 8);
  assert.equal(audit.chain_id, "chain");
});

test("getGovernanceCouncilAudit validates options and signal", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getGovernanceCouncilAudit("bad"),
    /getGovernanceCouncilAudit options must be an object/,
  );
  await assert.rejects(
    () => client.getGovernanceCouncilAudit(null),
    /getGovernanceCouncilAudit options must be an object/,
  );
  await assert.rejects(
    () => client.getGovernanceCouncilAudit({ signal: {} }),
    /getGovernanceCouncilAudit options\.signal must be an AbortSignal/,
  );
  await assert.rejects(
    () => client.getGovernanceCouncilAudit({ epoch: 1, extra: true }),
    /getGovernanceCouncilAudit options contains unsupported fields: extra/,
  );
});

test("governanceFinalizeReferendum validates required fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 204 }),
  });
  await assert.rejects(
    () =>
      client.governanceFinalizeReferendum({
        proposalId: `0x${"11".repeat(32)}`,
      }),
    (error) =>
      expectValidationErrorFixture(
        error,
        "governanceFinalizeReferendum_referendum_required",
      ),
  );
  await assert.rejects(
    () =>
      client.governanceFinalizeReferendum({
        referendumId: "ref-3",
      }),
    (error) =>
      expectValidationErrorFixture(
        error,
        "governanceFinalizeReferendum_proposal_required",
      ),
  );
  await assert.rejects(
    () =>
      client.governanceFinalizeReferendum({
        referendumId: "ref-3",
        proposalId: "beef",
      }),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_HEX);
      assert.equal(error.path, "governanceFinalizeReferendum.proposal_id");
      return true;
    },
  );
});

test("governanceEnactProposal validates proposal id and window bounds", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { ok: true },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () =>
      client.governanceEnactProposal({
        proposalId: "1234",
      }),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_HEX);
      assert.equal(error.path, "governanceEnactProposal.proposal_id");
      return true;
    },
  );
  await assert.rejects(
    () =>
      client.governanceEnactProposal({
        proposalId: `0x${"aa".repeat(32)}`,
        preimageHash: "zzzz",
      }),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_HEX);
      assert.equal(error.path, "governanceEnactProposal.preimage_hash");
      return true;
    },
  );
  await assert.rejects(
    () =>
      client.governanceEnactProposal({
        proposalId: `0x${"aa".repeat(32)}`,
        window: { upper: 4 },
      }),
    (error) =>
      expectValidationErrorFixture(
        error,
        "governanceEnactProposal_window_lower_required",
      ),
  );
  await assert.rejects(
    () =>
      client.governanceEnactProposal({
        proposalId: `0x${"aa".repeat(32)}`,
        window: { lower: 1 },
      }),
    (error) =>
      expectValidationErrorFixture(
        error,
        "governanceEnactProposal_window_upper_required",
      ),
  );
  await assert.rejects(
    () =>
      client.governanceEnactProposal({
        proposalId: `0x${"aa".repeat(32)}`,
        window: { lower: 5, upper: 4 },
      }),
    (error) =>
      expectValidationErrorFixture(error, "governanceEnactProposal_window_bounds"),
  );
});

test("governance helpers surface structured hex validation", async () => {
  const client = new ToriiClient(BASE_URL);
  const cases = [
    [
      "governanceFinalizeReferendum",
      () =>
        client.governanceFinalizeReferendum({
          referendumId: "ref-structured",
          proposalId: `zz${"aa".repeat(31)}`,
        }),
      "governanceFinalizeReferendum.proposal_id",
    ],
    [
      "governanceEnactProposal",
      () =>
        client.governanceEnactProposal({
          proposalId: "0x1234",
        }),
      "governanceEnactProposal.proposal_id",
    ],
  ];
  for (const [label, invoke, path] of cases) {
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(invoke, (error) => {
      assert(
        error instanceof ValidationError,
        `${label} should return a ValidationError for invalid proposal id`,
      );
      assert.equal(error.code, ValidationErrorCode.INVALID_HEX);
      assert.equal(error.path, path);
      assert.match(error.message, /32-byte hex string/);
      return true;
    });
  }
});

test("governance council candidate validation emits structured errors", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: () => {
      throw new Error("governanceDeriveCouncilVrf should not perform network requests");
    },
  });
  const validCandidate = {
    account_id: SAMPLE_ACCOUNT_ID,
    variant: "Normal",
    pk_b64: Buffer.from("candidate-pk").toString("base64"),
    proof_b64: Buffer.from("candidate-proof").toString("base64"),
  };
  await assert.rejects(
    () =>
      client.governanceDeriveCouncilVrf({
        committeeSize: 1,
        candidates: null,
      }),
    (error) => expectValidationErrorFixture(error, "governanceDeriveCouncilVrf_candidates_array"),
  );
  await assert.rejects(
    () =>
      client.governanceDeriveCouncilVrf({
        committeeSize: 1,
        candidates: [{ ...validCandidate, pk_b64: undefined }],
      }),
    (error) => expectValidationErrorFixture(error, "governanceDeriveCouncilVrf_candidate_pk"),
  );
  await assert.rejects(
    () =>
      client.governanceDeriveCouncilVrf({
        committeeSize: 1,
        candidates: [{ ...validCandidate, variant: "unknown" }],
      }),
    (error) => expectValidationErrorFixture(error, "governanceDeriveCouncilVrf_variant_value"),
  );
});

test("setProtectedNamespaces posts trimmed namespace list", async () => {
  let captured;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url, init) => {
      captured = { url, init };
      return createResponse({
        status: 200,
        jsonData: { ok: true, applied: 2 },
        headers: { "content-type": "application/json" },
      });
    },
  });
  const result = await client.setProtectedNamespaces([" apps ", "system"]);
  assert.deepEqual(result, { ok: true, applied: 2 });
  assert.equal(captured.url, `${BASE_URL}/v1/gov/protected-namespaces`);
  assert.equal(captured.init.method, "POST");
  assert.equal(captured.init.headers["Content-Type"], "application/json");
  assert.deepEqual(JSON.parse(captured.init.body), {
    namespaces: ["apps", "system"],
  });
});

test("setProtectedNamespaces validates namespace inputs", async () => {
  let called = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      called = true;
      return createResponse({
        status: 200,
        jsonData: { ok: true, applied: 0 },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await assert.rejects(() => client.setProtectedNamespaces([]), /must not be empty/);
  await assert.rejects(
    () => client.setProtectedNamespaces(["apps", 42]),
    /namespaces\[1\] must be a string/,
  );
  assert.equal(called, false);
});

test("getProtectedNamespaces normalizes payload", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { found: "true", namespaces: [" apps ", "system"] },
        headers: { "content-type": "application/json" },
      }),
  });
  const result = await client.getProtectedNamespaces();
  assert.deepEqual(result, { found: true, namespaces: ["apps", "system"] });
});

test("getSumeragiRbc returns null when telemetry disabled", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 503 }),
  });
  const payload = await client.getSumeragiRbc();
  assert.equal(payload, null);
});

test("getSumeragiRbc normalizes telemetry snapshot", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          sessions_active: 2,
          sessions_pruned_total: 1,
          ready_broadcasts_total: 5,
          deliver_broadcasts_total: 7,
          payload_bytes_delivered_total: 42,
        },
        headers: { "content-type": "application/json" },
      }),
  });
  const snapshot = await client.getSumeragiRbc();
  assert.deepEqual(snapshot, {
    sessionsActive: 2,
    sessionsPrunedTotal: 1,
    readyBroadcastsTotal: 5,
    deliverBroadcastsTotal: 7,
    payloadBytesDeliveredTotal: 42,
  });
});

test("getSumeragiRbc enforces options object", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 503 }),
  });
  await assert.rejects(
    () => client.getSumeragiRbc(42),
    /getSumeragiRbc options must be an object/,
  );
});

test("getSumeragiRbc threads AbortSignal to fetch request", async () => {
  const controller = new AbortController();
  let capturedSignal;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (_url, init = {}) => {
      capturedSignal = init.signal;
      return createResponse({
        status: 200,
        jsonData: {
          sessions_active: 0,
          sessions_pruned_total: 0,
          ready_broadcasts_total: 0,
          deliver_broadcasts_total: 0,
          payload_bytes_delivered_total: 0,
        },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await client.getSumeragiRbc({ signal: controller.signal });
  assert.equal(capturedSignal, controller.signal);
});

test("getSumeragiRbcSessions normalizes payload", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          sessions_active: 1,
          items: [
            {
              block_hash: "abc",
              height: 10,
              view: 1,
              total_chunks: 4,
              received_chunks: 2,
              ready_count: 3,
              delivered: false,
              invalid: false,
              payload_hash: null,
              recovered: true,
            },
          ],
        },
        headers: { "content-type": "application/json" },
      }),
  });
  const sessions = await client.getSumeragiRbcSessions();
  assert.equal(sessions?.sessionsActive, 1);
  assert.equal(sessions?.items.length, 1);
  assert.deepEqual(sessions?.items[0], {
    blockHash: "abc",
    height: 10,
    view: 1,
    totalChunks: 4,
    receivedChunks: 2,
    readyCount: 3,
    delivered: false,
    invalid: false,
    payloadHash: null,
    recovered: true,
  });
});

test("getSumeragiRbcSessions enforces options object", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 503 }),
  });
  await assert.rejects(
    () => client.getSumeragiRbcSessions(42),
    /getSumeragiRbcSessions options must be an object/,
  );
});

test("findRbcSamplingCandidate returns null when telemetry disabled", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 503 }),
  });
  const candidate = await client.findRbcSamplingCandidate();
  assert.equal(candidate, null);
});

test("findRbcSamplingCandidate returns delivered session", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          sessions_active: 2,
          items: [
            {
              block_hash: null,
              height: 15,
              view: 0,
              total_chunks: 2,
              received_chunks: 1,
              ready_count: 1,
              delivered: true,
              invalid: false,
              payload_hash: null,
              recovered: true,
            },
            {
              block_hash: "deadbeef",
              height: 16,
              view: 1,
              total_chunks: 4,
              received_chunks: 3,
              ready_count: 4,
              delivered: true,
              invalid: false,
              payload_hash: "cafebabe",
              recovered: false,
            },
          ],
        },
        headers: { "content-type": "application/json" },
      }),
  });
  const candidate = await client.findRbcSamplingCandidate();
  assert.deepEqual(candidate, {
    blockHash: "deadbeef",
    height: 16,
    view: 1,
    totalChunks: 4,
    receivedChunks: 3,
    readyCount: 4,
    delivered: true,
    invalid: false,
    payloadHash: "cafebabe",
    recovered: false,
  });
});

test("findRbcSamplingCandidate enforces AbortSignal option", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 503 }),
  });
  await assert.rejects(
    () => client.findRbcSamplingCandidate({ signal: {} }),
    /findRbcSamplingCandidate options\.signal must be an AbortSignal/,
  );
});

test("findRbcSamplingCandidate threads AbortSignal to snapshot request", async () => {
  const controller = new AbortController();
  let capturedSignal;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (_url, init = {}) => {
      capturedSignal = init.signal;
      return createResponse({
        status: 200,
        jsonData: { sessions_active: 0, items: [] },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await client.findRbcSamplingCandidate({ signal: controller.signal });
  assert.equal(capturedSignal, controller.signal);
});

test("getSumeragiRbcDelivered normalizes payload", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          height: 11,
          view: 3,
          delivered: true,
          present: true,
          block_hash: "deadbeef",
          ready_count: 5,
          received_chunks: 4,
          total_chunks: 6,
        },
        headers: { "content-type": "application/json" },
      }),
  });
  const delivered = await client.getSumeragiRbcDelivered(11, 3);
  assert.deepEqual(delivered, {
    height: 11,
    view: 3,
    delivered: true,
    present: true,
    blockHash: "deadbeef",
    readyCount: 5,
    receivedChunks: 4,
    totalChunks: 6,
  });
});

test("getSumeragiRbcSessions rejects unsupported options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 200, jsonData: { items: [] } }),
  });
  await assert.rejects(
    () =>
      client.getSumeragiRbcSessions({
        signal: new AbortController().signal,
        extra: "nope",
      }),
    /getSumeragiRbcSessions options contains unsupported fields: extra/,
  );
});

test("getSumeragiRbcDelivered enforces options object", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 404 }),
  });
  await assert.rejects(
    () => client.getSumeragiRbcDelivered(1, 0, 42),
    /getSumeragiRbcDelivered options must be an object/,
  );
});

test("getSumeragiRbcDelivered rejects unsupported options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 404 }),
  });
  await assert.rejects(
    () =>
      client.getSumeragiRbcDelivered(1, 0, {
        signal: new AbortController().signal,
        extra: true,
      }),
    /getSumeragiRbcDelivered options contains unsupported fields: extra/,
  );
});

test("getSumeragiRbcDelivered threads AbortSignal to fetch request", async () => {
  const controller = new AbortController();
  let capturedSignal;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (_url, init = {}) => {
      capturedSignal = init.signal;
      return createResponse({
        status: 404,
        headers: { "content-type": "application/json" },
      });
    },
  });
  await client.getSumeragiRbcDelivered(1, 0, { signal: controller.signal });
  assert.equal(capturedSignal, controller.signal);
});

test("findRbcSamplingCandidate rejects unsupported options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { items: [] },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () =>
      client.findRbcSamplingCandidate({
        signal: new AbortController().signal,
        unexpected: true,
      }),
    /findRbcSamplingCandidate options contains unsupported fields: unexpected/,
  );
});

test("sampleRbcChunks posts payload and handles 404", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({ status: 404 });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl, allowInsecure: true });
  const result = await client.sampleRbcChunks({
    blockHash: "abcd1234",
    height: 42,
    view: 0,
    count: 2,
    seed: 7,
    apiToken: "token",
  });
  assert.equal(result, null);
  assert.equal(captured.url, `${BASE_URL}/v1/sumeragi/rbc/sample`);
  const parsed = JSON.parse(captured.init.body);
  assert.equal(parsed.block_hash, "abcd1234");
  assert.equal(parsed.count, 2);
  assert.equal(parsed.seed, 7);
  assert.equal(captured.init.headers["X-API-Token"], "token");
});

test("sampleRbcChunks normalizes sample payload", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          block_hash: "cafebabe",
          height: 12,
          view: 0,
          total_chunks: 8,
          chunk_root: "deadbeef00",
          payload_hash: null,
          samples: [
            {
              index: 1,
              chunk_hex: "aa",
              digest_hex: "bb",
              proof: {
                leaf_index: 1,
                depth: 2,
                audit_path: ["ff", null],
              },
            },
          ],
        },
        headers: { "content-type": "application/json" },
      }),
  });
  const sample = await client.sampleRbcChunks({
    blockHash: "cafebabe",
    height: 12,
    view: 0,
  });
  assert.equal(sample?.blockHash, "cafebabe");
  assert.equal(sample?.samples.length, 1);
  assert.deepEqual(sample?.samples[0].proof.auditPath, ["ff", null]);
});

test("sampleRbcChunks rejects malformed hex fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          block_hash: "0xz-not-hex",
          height: 1,
          view: 0,
          total_chunks: 1,
          chunk_root: "deadbeef00",
          payload_hash: null,
          samples: [],
        },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () =>
      client.sampleRbcChunks({
        blockHash: "deadbeef",
        height: 1,
        view: 0,
      }),
    /sumeragi rbc sample response\.block_hash/,
  );
});

test("sampleRbcChunks rejects unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 404 }),
  });
  await assert.rejects(
    () =>
      client.sampleRbcChunks({
        blockHash: "abcd",
        height: 1,
        view: 0,
        note: "extra",
      }),
    /sampleRbcChunks options contains unsupported fields: note/,
  );
});

test("sampleRbcChunks forwards AbortSignal option", async () => {
  let observedSignal;
  const controller = new AbortController();
  const fetchImpl = async (_url, init) => {
    observedSignal = init.signal;
    return createResponse({ status: 404 });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.sampleRbcChunks({
    blockHash: "cafebabe",
    height: 1,
    view: 0,
    signal: controller.signal,
  });
  assert.equal(observedSignal, controller.signal);
});

test("sampleRbcChunks reuses client apiToken header", async () => {
  let captured;
  const fetchImpl = async (_url, init) => {
    captured = init;
    return createResponse({ status: 404 });
  };
  const client = new ToriiClient(BASE_URL, {
    fetchImpl,
    apiToken: "client-token",
    allowInsecure: true,
  });
  await client.sampleRbcChunks({
    blockHash: "abcd",
    height: 1,
    view: 0,
  });
  assert.equal(captured.headers["X-API-Token"], "client-token");
  assert.equal(captured.headers["X-Iroha-API-Token"], undefined);
});

test("buildRbcSampleRequest derives payload from session", () => {
  const session = sampleRbcSession({ blockHash: "deadbeef" });
  const request = buildRbcSampleRequest(session, {
    count: 2,
    seed: 0,
    apiToken: "token-123",
  });
  assert.deepEqual(request, {
    blockHash: "deadbeef",
    height: 12,
    view: 3,
    count: 2,
    seed: 0,
    apiToken: "token-123",
  });
});

test("buildRbcSampleRequest rejects sessions without block hash", () => {
  const session = sampleRbcSession({ blockHash: null });
  assert.throws(() => buildRbcSampleRequest(session), /blockHash must be a hex string/);
});

test("buildRbcSampleRequest enforces override objects", () => {
  const session = sampleRbcSession();
  assert.throws(() => buildRbcSampleRequest(session, null), /overrides must be an object/);
});

test("buildRbcSampleRequest emits structured validation errors", () => {
  assert.throws(
    () => buildRbcSampleRequest(null),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_OBJECT);
      assert.equal(error.path, "buildRbcSampleRequest.session");
      assert.match(error.message, /session must be an object/);
      return true;
    },
  );
  const session = sampleRbcSession();
  assert.throws(
    () => buildRbcSampleRequest(session, null),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_OBJECT);
      assert.equal(error.path, "buildRbcSampleRequest.overrides");
      assert.match(error.message, /overrides must be an object/);
      return true;
    },
  );
});

test("getSumeragiRbcDelivered returns null on 404", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 404 }),
  });
  const delivered = await client.getSumeragiRbcDelivered(10, 1);
  assert.equal(delivered, null);
});

test("listSumeragiEvidence encodes query parameters", async () => {
  let observedSignal;
  const fetchImpl = async (url, init) => {
    assert.equal(
      url,
      `${BASE_URL}/v1/sumeragi/evidence?limit=25&offset=5&kind=DoublePrepare`,
    );
    assert.equal(init.headers.Accept, "application/json");
    observedSignal = init.signal;
    assert.ok(observedSignal instanceof AbortSignal);
    return createResponse({
      status: 200,
      jsonData: {
        total: 1,
        items: [
          {
            kind: "DoublePrepare",
            phase: "Prepare",
            height: 10,
            view: 2,
            epoch: 1,
            signer: "alice@test",
            block_hash_1: "aa",
            block_hash_2: "bb",
            recorded_height: 10,
            recorded_view: 2,
            recorded_ms: 123,
          },
        ],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const controller = new AbortController();
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.listSumeragiEvidence({
    limit: 25,
    offset: 5,
    kind: "DoublePrepare",
    signal: controller.signal,
  });
  assert.equal(payload.total, 1);
  assert.equal(payload.items.length, 1);
  assert.equal(payload.items[0].kind, "DoublePrepare");
  controller.abort();
  assert.ok(observedSignal?.aborted);
});

test("listSumeragiEvidence rejects invalid kind", async () => {
  const client = new ToriiClient(BASE_URL, { fetchImpl: async () => createResponse({ status: 200 }) });
  await assert.rejects(
    () => client.listSumeragiEvidence({ kind: "Invalid" }),
    /kind must be one of/,
  );
});

test("listSumeragiEvidence rejects unsupported options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 200, jsonData: { total: 0, items: [] } }),
  });
  await assert.rejects(
    () =>
      client.listSumeragiEvidence({
        kind: "DoublePrepare",
        limit: 1,
        note: "extra",
      }),
    /listSumeragiEvidence options contains unsupported fields: note/,
  );
});

test("listSumeragiEvidence normalizes evidence payloads", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        total: 10,
        items: [
          {
            kind: "DoublePrepare",
            phase: "Prepare",
            height: "42",
            view: "7",
            epoch: "3",
            signer: "alice@test",
            block_hash_1: "aa",
            block_hash_2: "bb",
            recorded_height: 80,
            recorded_view: 1,
            recorded_ms: 1234,
          },
          {
            kind: "Censorship",
            tx_hash: "44",
            receipt_count: 3,
            min_height: 10,
            max_height: 12,
            signers: ["alice@test", "bob@test"],
            recorded_height: 81,
            recorded_view: 3,
            recorded_ms: 1500,
          },
          {
            kind: "InvalidQc",
            height: 2,
            view: 3,
            epoch: 4,
            subject_block_hash: "11",
            phase: "Commit",
            reason: "bad qc",
            recorded_height: 82,
            recorded_view: 4,
            recorded_ms: 1600,
          },
          {
            kind: "InvalidProposal",
            height: 6,
            view: 7,
            epoch: 8,
            subject_block_hash: "22",
            payload_hash: "33",
            reason: "bad payload",
            recorded_height: 83,
            recorded_view: 5,
            recorded_ms: 1700,
          },
          {
            kind: "UnknownEvidence",
            detail: "unsupported",
            recorded_height: 84,
            recorded_view: 6,
            recorded_ms: 1800,
          },
        ],
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.listSumeragiEvidence();
  assert.equal(payload.total, 10);
  assert.deepEqual(payload.items, [
    {
      kind: "DoublePrepare",
      recorded_height: 80,
      recorded_view: 1,
      recorded_ms: 1234,
      phase: "Prepare",
      height: 42,
      view: 7,
      epoch: 3,
      signer: "alice@test",
      block_hash_1: "aa",
      block_hash_2: "bb",
    },
    {
      kind: "Censorship",
      recorded_height: 81,
      recorded_view: 3,
      recorded_ms: 1500,
      tx_hash: "44",
      receipt_count: 3,
      min_height: 10,
      max_height: 12,
      signers: ["alice@test", "bob@test"],
    },
    {
      kind: "InvalidQc",
      recorded_height: 82,
      recorded_view: 4,
      recorded_ms: 1600,
      height: 2,
      view: 3,
      epoch: 4,
      subject_block_hash: "11",
      phase: "Commit",
      reason: "bad qc",
    },
    {
      kind: "InvalidProposal",
      recorded_height: 83,
      recorded_view: 5,
      recorded_ms: 1700,
      height: 6,
      view: 7,
      epoch: 8,
      subject_block_hash: "22",
      payload_hash: "33",
      reason: "bad payload",
    },
    {
      kind: "UnknownEvidence",
      recorded_height: 84,
      recorded_view: 6,
      recorded_ms: 1800,
      detail: "unsupported",
    },
  ]);
});

test("listSumeragiEvidence rejects malformed payloads", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        total: 1,
        items: [
          {
            kind: "DoublePrepare",
            phase: "Prepare",
            height: 1,
            view: 0,
            epoch: 0,
            signer: "alice@test",
            block_hash_1: "aa",
            block_hash_2: "bb",
            recorded_view: 0,
            recorded_ms: 0,
          },
        ],
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(() => client.listSumeragiEvidence(), /recorded_height/);
});

test("getSumeragiEvidenceCount returns count payload", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: { count: 7 },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getSumeragiEvidenceCount();
  assert.deepEqual(result, { count: 7 });
});

test("submitSumeragiEvidence posts body and returns response", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 202,
      jsonData: { status: "accepted", kind: "DoublePrepare" },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl, allowInsecure: true });
  const response = await client.submitSumeragiEvidence({
    evidence_hex: "deadbeef",
    apiToken: "s3cret",
  });
  assert.equal(captured.url, `${BASE_URL}/v1/sumeragi/evidence/submit`);
  assert.equal(captured.init.headers["X-API-Token"], "s3cret");
  assert.equal(JSON.parse(captured.init.body).evidence_hex, "deadbeef");
  assert.deepEqual(response, { status: "accepted", kind: "DoublePrepare" });
});

test("getMetrics returns text when requested", async () => {
  const metrics = "# HELP foo\nfoo 1\n";
  const fetchImpl = async (_url, init) => {
    assert.equal(init.headers.Accept, "text/plain");
    return createResponse({
      status: 200,
      textBody: metrics,
      headers: { "content-type": "text/plain" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.getMetrics({ asText: true });
  assert.equal(payload, metrics);
});

test("getMetrics returns JSON by default", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/metrics`);
    assert.equal(init.headers.Accept, "application/json");
    return createResponse({
      status: 200,
      jsonData: { metrics: ["ok"] },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.getMetrics();
  assert.deepEqual(payload, { metrics: ["ok"] });
});

test("getMetrics rejects non-object options", async () => {
  const fetchImpl = async () => {
    throw new Error("should not reach fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(client.getMetrics("nope"), /getMetrics options must be an object/);
});

test("getMetrics enforces boolean asText flag", async () => {
  const fetchImpl = async () => {
    throw new Error("should not reach fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    client.getMetrics({ asText: "true" }),
    /getMetrics options\.asText must be boolean/,
  );
});

test("getMetrics rejects unsupported option keys", async () => {
  const fetchImpl = async () => {
    throw new Error("should not reach fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getMetrics({ asText: true, extra: true }),
    /getMetrics options contains unsupported fields: extra/,
  );
});

test("getMetrics forwards AbortSignal", async () => {
  const controller = new AbortController();
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/metrics`);
    assert.equal(init.headers.Accept, "application/json");
    assert.strictEqual(init.signal, controller.signal);
    return createResponse({
      status: 200,
      jsonData: { ok: true },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.getMetrics({ signal: controller.signal });
  assert.deepEqual(payload, { ok: true });
});

test("getBlock fetches block by height", async () => {
  const fetchImpl = async (url) => {
    assert.equal(url, `${BASE_URL}/v1/blocks/42`);
    return createResponse({
      status: 200,
      jsonData: {
        hash: "DEADBEEF",
        height: 42,
        created_at: "2026-01-01T00:00:00Z",
        prev_block_hash: null,
        transactions_hash: "ABCD",
        transactions_rejected: 1,
        transactions_total: 5,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const block = await client.getBlock(42);
  assert.deepEqual(block, {
    hash: "DEADBEEF",
    height: 42,
    createdAt: "2026-01-01T00:00:00Z",
    prevBlockHash: null,
    transactionsHash: "ABCD",
    transactionsRejected: 1,
    transactionsTotal: 5,
  });
});

test("getBlock forwards AbortSignal", async () => {
  const controller = new AbortController();
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/blocks/7`);
    assert.strictEqual(init.signal, controller.signal);
    return createResponse({
      status: 200,
      jsonData: {
        hash: "aa",
        height: 7,
        created_at: "now",
        prev_block_hash: null,
        transactions_hash: "bb",
        transactions_rejected: 0,
        transactions_total: 0,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const block = await client.getBlock(7, { signal: controller.signal });
  assert.equal(block.height, 7);
});

test("getBlock returns null when Torii replies 404", async () => {
  const fetchImpl = async () => {
    return createResponse({
      status: 404,
      jsonData: null,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const block = await client.getBlock(9999);
  assert.equal(block, null);
});

test("listBlocks encodes pagination parameters", async () => {
  const fetchImpl = async (url) => {
    assert.equal(url, `${BASE_URL}/v1/blocks?offset_height=10&limit=5`);
    return createResponse({
      status: 200,
      jsonData: {
        pagination: {
          page: 1,
          per_page: 5,
          total_pages: 2,
          total_items: 8,
        },
        items: [
          {
            hash: "CAFE",
            height: 8,
            created_at: "2026-01-01T00:00:00Z",
            prev_block_hash: "BEEF",
            transactions_hash: null,
            transactions_rejected: 0,
            transactions_total: 4,
          },
        ],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.listBlocks({ offsetHeight: 10, limit: 5 });
  assert.deepEqual(result, {
    pagination: {
      page: 1,
      perPage: 5,
      totalPages: 2,
      totalItems: 8,
    },
    items: [
      {
        hash: "CAFE",
        height: 8,
        createdAt: "2026-01-01T00:00:00Z",
        prevBlockHash: "BEEF",
        transactionsHash: null,
        transactionsRejected: 0,
        transactionsTotal: 4,
      },
    ],
  });
});

test("getBlock rejects invalid heights", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getBlock(-1),
    /non-negative integer/,
  );
});

test("getBlock rejects unsupported option keys", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getBlock(1, { unexpected: true }),
    /getBlock options contains unsupported fields: unexpected/,
  );
});

test("listBlocks validates pagination bounds", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listBlocks({ limit: 0 }),
    /positive integer/,
  );
  await assert.rejects(
    () => client.listBlocks({ offsetHeight: -5 }),
    /non-negative integer/,
  );
});

test("listBlocks rejects non-object options", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listBlocks("oops"),
    /block list options must be a plain object/,
  );
});

test("listBlocks rejects unsupported option keys", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listBlocks({ unexpected: true }),
    /block list options contains unsupported fields: unexpected/,
  );
});

test("listAccounts encodes iterable params", async () => {
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/accounts");
    assert.equal(parsed.searchParams.get("limit"), "10");
    assert.equal(parsed.searchParams.get("offset"), "5");
    assert.equal(
      parsed.searchParams.get("filter"),
      JSON.stringify({ Eq: ["id", FIXTURE_ALICE_ID] }),
    );
    assert.equal(parsed.searchParams.get("sort"), "id:asc");
    assert.equal(parsed.searchParams.get("canonical_i105"), null);
    return createResponse({
      status: 200,
      jsonData: cloneFixture(toriiFixtures.iterable.accountListPage),
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.listAccounts({
    limit: "10",
    offset: 5n,
    filter: { Eq: ["id", FIXTURE_ALICE_ID] },
    sort: [{ key: "id", order: "asc" }],
  });
  assert.deepEqual(payload, toriiFixtures.iterable.accountListPage);
});

test("listAccounts rejects unsupported legacy option", async () => {
  let called = false;
  const fetchImpl = async () => {
    called = true;
    return createResponse({
      status: 200,
      jsonData: { items: [], total: 0 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listAccounts({ legacyFormat: "i105" }),
    /unsupported fields: legacyFormat/i,
  );
  assert.equal(called, false, "request should not fire when legacyFormat is unsupported");
});

test("listAccounts rejects unsupported sort order entries", async () => {
  let called = false;
  const fetchImpl = async () => {
    called = true;
    return createResponse({ status: 200, jsonData: { items: [], total: 0 } });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () =>
      client.listAccounts({
        sort: [{ key: "id", order: "ascending" }],
      }),
    /sort\[0]\.order must be "asc" or "desc"/,
  );
  assert.equal(called, false);
});

test("listAccounts rejects non-object filter values", async () => {
  let callCount = 0;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      callCount += 1;
      return createResponse({
        status: 200,
        jsonData: { items: [], total: 0 },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await assert.rejects(
    () => client.listAccounts({ filter: [] }),
    /filter must be a plain object/,
  );
  assert.equal(callCount, 0);
});

test("listAccounts rejects primitive options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not call fetch");
    },
  });
  await assert.rejects(
    client.listAccounts("bogus"),
    /options for \/v1\/accounts must be a plain object/,
  );
});

test("listAccounts rejects unsupported iterable option keys", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not call fetch");
    },
  });
  await assert.rejects(
    () => client.listAccounts({ limit: 1, unknown: true }),
    /options for \/v1\/accounts contains unsupported fields: unknown/,
  );
});

test("listAccounts rejects query-only iterable options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not call fetch");
    },
  });
  await assert.rejects(
    () => client.listAccounts({ fetchSize: 5 }),
    /options for \/v1\/accounts contains unsupported fields: fetchSize/,
  );
});

test("listAccounts validates response payload IDs", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { items: [{}], total: 1 },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.listAccounts(),
    /account list response\.items\[0]\.id/,
  );
});

test("queryAccounts rejects primitive options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not call fetch");
    },
  });
  await assert.rejects(
    client.queryAccounts("bogus"),
    /options for \/v1\/accounts\/query must be a plain object/,
  );
});

test("queryAccounts rejects non-query iterable fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not call fetch");
    },
  });
  await assert.rejects(
    () => client.queryAccounts({ controllerId: FIXTURE_ALICE_ID }),
    /options for \/v1\/accounts\/query contains unsupported fields: controllerId/,
  );
});

test("queryAccounts rejects array filters from JSON strings", async () => {
  let callCount = 0;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      callCount += 1;
      return createResponse({
        status: 200,
        jsonData: { items: [], total: 0 },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await assert.rejects(
    () => client.queryAccounts({ filter: "[]" }),
    /filter must be a plain object/,
  );
  assert.equal(callCount, 0);
});

test("queryAccounts rejects unsupported legacy option", async () => {
  let captured;
  const fetchImpl = async (_url, init) => {
    captured = JSON.parse(init.body);
    return createResponse({
      status: 200,
      jsonData: { items: [], total: 0 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.queryAccounts({ legacyFormat: "i105" }),
    /unsupported fields: legacyFormat/i,
  );
  assert.equal(captured, undefined);
});

test("queryAccounts rejects unsupported sort order tokens", async () => {
  let callCount = 0;
  const fetchImpl = async () => {
    callCount += 1;
    return createResponse({ status: 200, jsonData: { items: [], total: 0 } });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.queryAccounts({ sort: "id:descendingly" }),
    /sort token at index 0 order must be "asc" or "desc"/,
  );
  assert.equal(callCount, 0);
});

test("queryDomains rejects non-object select entries", async () => {
  let callCount = 0;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      callCount += 1;
      return createResponse({
        status: 200,
        jsonData: { items: [], total: 0 },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await assert.rejects(
    () =>
      client.queryDomains({
        select: [{ id: true }, []],
      }),
    /select\[1] must be a plain object/,
  );
  assert.equal(callCount, 0);
});

test("listNfts hits nft endpoint", async () => {
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/nfts");
    assert.equal(parsed.searchParams.get("limit"), "25");
    return createResponse({
      status: 200,
      jsonData: { items: [{ id: "nft#1" }], total: 1 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.listNfts({ limit: 25 });
  assert.deepEqual(payload.items[0], { id: "nft#1" });
});

test("listExplorerNfts encodes owner/domain filters and pagination", async () => {
  const calls = [];
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    calls.push(parsed);
    return createResponse({
      status: 200,
      jsonData: {
        pagination: { page: 2, per_page: 5, total_pages: 3, total_items: 12 },
        items: [
          { id: "6HptcdrgYMsS3ARWDMaabCQJtqQd#1", owned_by: SAMPLE_ACCOUNT_ID, metadata: { role: "demo" } },
        ],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const page = await client.listExplorerNfts({
    ownedBy: SAMPLE_ACCOUNT_ID,
    domainId: "wonderland",
    offset: 5,
    limit: 5,
  });
  assert.equal(calls.length, 1);
  const parsed = calls[0];
  assert.equal(parsed.pathname, "/v1/explorer/nfts");
  assert.equal(parsed.searchParams.get("owned_by"), SAMPLE_ACCOUNT_ID);
  assert.equal(parsed.searchParams.get("domain"), "wonderland");
  assert.equal(parsed.searchParams.get("per_page"), "5");
  assert.equal(parsed.searchParams.get("page"), "2");
  assert.equal(parsed.searchParams.get("canonical_i105"), null);
  assert.deepEqual(page.pagination, { page: 2, perPage: 5, totalPages: 3, totalItems: 12 });
  assert.deepEqual(page.items[0], {
    id: "6HptcdrgYMsS3ARWDMaabCQJtqQd#1",
    ownedBy: SAMPLE_ACCOUNT_ID,
    metadata: { role: "demo" },
  });
});

test("iterateAccountNfts walks explorer pagination and honours maxItems", async () => {
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    const page = Number(parsed.searchParams.get("page") ?? 1);
    const perPage = Number(parsed.searchParams.get("per_page") ?? 10);
    const totalItems = 5;
    const start = (page - 1) * perPage;
    const remaining = Math.max(0, totalItems - start);
    const items = Array.from({ length: Math.min(perPage, remaining) }, (_, index) => ({
      id: `6HptcdrgYMsS3ARWDMaabCQJtqQd#${start + index + 1}`,
      owned_by: SAMPLE_ACCOUNT_ID,
      metadata: { page, perPage },
    }));
    const totalPages = Math.ceil(totalItems / perPage);
    return createResponse({
      status: 200,
      jsonData: {
        pagination: {
          page,
          per_page: perPage,
          total_pages: totalPages,
          total_items: totalItems,
        },
        items,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const seen = [];
  for await (const nft of client.iterateAccountNfts(SAMPLE_ACCOUNT_ID, {
    pageSize: 2,
    maxItems: 3,
  })) {
    seen.push(nft.id);
  }
  assert.deepEqual(seen, ["6HptcdrgYMsS3ARWDMaabCQJtqQd#1", "6HptcdrgYMsS3ARWDMaabCQJtqQd#2", "6HptcdrgYMsS3ARWDMaabCQJtqQd#3"]);
});

test("listExplorerNfts surfaces permission errors", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 403,
      jsonData: { error: "forbidden" },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listExplorerNfts(),
    (error) => error instanceof ToriiHttpError && error.status === 403,
  );
});

test("queryDomains posts structured envelope", async () => {
  let capturedBody;
  const fetchImpl = async (_url, init) => {
    assert.equal(init.method, "POST");
    assert.equal(init.headers["Content-Type"], "application/json");
    capturedBody = JSON.parse(init.body);
    return createResponse({
      status: 200,
      jsonData: { items: [], total: 0 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.queryDomains({
    filter: { Eq: ["id", "wonderland"] },
    sort: "metadata.display_name:desc",
    fetchSize: "32",
    queryName: "FindDomains",
    select: [{ id: true }],
  });
  assert.deepEqual(capturedBody.pagination, { offset: 0 });
  assert.deepEqual(capturedBody.filter, { Eq: ["id", "wonderland"] });
  assert.deepEqual(capturedBody.sort, [
    { key: "metadata.display_name", order: "desc" },
  ]);
  assert.equal(capturedBody.fetch_size, 32);
  assert.equal(capturedBody.query, "FindDomains");
  assert.deepEqual(capturedBody.select, [{ id: true }]);
});

test("queryNfts posts Norito envelope", async () => {
  let capturedBody;
  const fetchImpl = async (_url, init) => {
    assert.equal(init.method, "POST");
    capturedBody = JSON.parse(init.body);
    return createResponse({
      status: 200,
      jsonData: { items: [], total: 0 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.queryNfts({
    filter: { Eq: ["id", "6HptcdrgYMsS3ARWDMaabCQJtqQd"] },
    sort: [{ key: "id", order: "desc" }],
    fetchSize: 10,
  });
  assert.deepEqual(capturedBody.filter, { Eq: ["id", "6HptcdrgYMsS3ARWDMaabCQJtqQd"] });
  assert.deepEqual(capturedBody.sort, [{ key: "id", order: "desc" }]);
  assert.equal(capturedBody.fetch_size, 10);
  assert.equal(capturedBody.canonical_i105, undefined);
});

test("listNfts enforces credentials when requirePermissions is set", async () => {
  let called = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      called = true;
      return createResponse({
        status: 200,
        jsonData: { items: [], total: 0 },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await assert.rejects(
    () => client.listNfts({ requirePermissions: true }),
    /listNfts requires authToken or apiToken/,
  );
  assert.equal(called, false);
});

test("listNfts accepts requirePermissions when credentials are present", async () => {
  let callCount = 0;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      callCount += 1;
      return createResponse({
        status: 200,
        jsonData: { items: [{ id: "nft#1" }], total: 1 },
        headers: { "content-type": "application/json" },
      });
    },
    authToken: "token",
  });
  const payload = await client.listNfts({ requirePermissions: true, limit: 1 });
  assert.equal(callCount, 1);
  assert.deepEqual(payload.items[0], { id: "nft#1" });
});

test("listNfts rejects non-boolean requirePermissions", async () => {
  let fetchCalled = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      fetchCalled = true;
      return createResponse({
        status: 200,
        jsonData: { items: [], total: 0 },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await assert.rejects(
    () =>
      client.listNfts({
        // @ts-expect-error runtime guard
        requirePermissions: "yes",
      }),
    /listNfts\.requirePermissions must be a boolean/,
  );
  assert.equal(fetchCalled, false);
});

test("iterateAccountsQuery paginates structured filters", async () => {
  let callCount = 0;
  const fetchImpl = async (url, init) => {
    assert.equal(init.method, "POST");
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/accounts/query");
    const body = JSON.parse(init.body);
    assert.deepEqual(body.filter, { Eq: ["id", SAMPLE_ACCOUNT_FORMS.i105Default] });
    const offset = Number(body.pagination?.offset ?? 0);
    const limit = Number(body.pagination?.limit ?? 0);
    if (callCount === 0) {
      assert.equal(limit, 2);
      assert.equal(offset, 0);
    } else {
      assert.equal(limit, 2);
      assert.equal(offset, 2);
    }
    callCount += 1;
    const items =
      offset === 0
        ? [{ id: "acc-1" }, { id: "acc-2" }]
        : [{ id: "acc-3" }];
    return createResponse({
      status: 200,
      jsonData: { items, total: 3 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const seen = [];
  for await (const account of client.iterateAccountsQuery({
    filter: { Eq: ["id", SAMPLE_ACCOUNT_FORMS.i105Default] },
    pageSize: 2,
  })) {
    seen.push(account.id);
  }
  assert.deepEqual(seen, ["acc-1", "acc-2", "acc-3"]);
  assert.equal(callCount, 2);
});

test("iterateAccounts rejects primitive iterator options", () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not call fetch");
    },
  });
  assert.throws(
    () => client.iterateAccounts("bogus"),
    /listAccounts iterator options must be a plain object/,
  );
});

test("iterateAccountsQuery rejects primitive iterator options", () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not call fetch");
    },
  });
  assert.throws(
    () => client.iterateAccountsQuery("bogus"),
    /queryAccounts iterator options must be a plain object/,
  );
});

test("iterateDomainsQuery pages through query endpoint", async () => {
  let callCount = 0;
  const fetchImpl = async (url, init) => {
    assert.equal(new URL(url).pathname, "/v1/domains/query");
    const body = JSON.parse(init.body);
    const offset = Number(body.pagination?.offset ?? 0);
    callCount += 1;
    const items = offset === 0 ? [{ id: "wonderland" }] : [{ id: "utopia" }];
    return createResponse({
      status: 200,
      jsonData: { items, total: 2 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const ids = [];
  for await (const domain of client.iterateDomainsQuery({ pageSize: 1 })) {
    ids.push(domain.id);
  }
  assert.deepEqual(ids, ["wonderland", "utopia"]);
  assert.equal(callCount, 2);
});

test("iterateAssetDefinitions advances pages and honours maxItems", async () => {
  const responses = [
    { items: [{ id: "a" }, { id: "b" }], total: 5 },
    { items: [{ id: "c" }], total: 5 },
  ];
  let callCount = 0;
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    if (callCount === 0) {
      assert.equal(parsed.searchParams.get("limit"), "2");
      assert.equal(parsed.searchParams.get("offset"), "0");
    } else if (callCount === 1) {
      assert.equal(parsed.searchParams.get("limit"), "1");
      assert.equal(parsed.searchParams.get("offset"), "2");
    }
    const payload = responses[callCount] ?? { items: [], total: 5 };
    callCount += 1;
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const collected = [];
  for await (const item of client.iterateAssetDefinitions({
    pageSize: 2,
    maxItems: 3,
  })) {
    collected.push(item.id);
  }
  assert.deepEqual(collected, ["a", "b", "c"]);
  assert.equal(callCount, 2);
});

test("iterateAssetDefinitionsQuery paginates query responses", async () => {
  let callCount = 0;
  const fetchImpl = async (url, init) => {
    assert.equal(new URL(url).pathname, "/v1/assets/definitions/query");
    const body = JSON.parse(init.body);
    const offset = Number(body.pagination?.offset ?? 0);
    callCount += 1;
    const items =
      offset === 0
        ? [{ id: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM" }]
        : [{ id: "6sfXUWFsj5B9CV4dXLq6nkU3H55W" }];
    return createResponse({
      status: 200,
      jsonData: { items, total: 2 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const ids = [];
  for await (const def of client.iterateAssetDefinitionsQuery({ pageSize: 1 })) {
    ids.push(def.id);
  }
  assert.deepEqual(ids, ["62Fk4FPcMuLvW5QjDGNF2a4jAmjM", "6sfXUWFsj5B9CV4dXLq6nkU3H55W"]);
  assert.equal(callCount, 2);
});

test("queryAssetDefinitions enforces requirePermissions", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not call fetch");
    },
  });
  await assert.rejects(
    () => client.queryAssetDefinitions({ requirePermissions: true }),
    /queryAssetDefinitions requires authToken or apiToken/,
  );
});

test("iterateNfts paginates across responses", async () => {
  const responses = [
    { items: [{ id: "nft#1" }], total: 3 },
    { items: [{ id: "nft#2" }], total: 3 },
    { items: [{ id: "nft#3" }], total: 3 },
  ];
  let callCount = 0;
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/nfts");
    assert.equal(parsed.searchParams.get("limit"), "1");
    assert.equal(parsed.searchParams.get("offset"), String(callCount));
    const payload = responses[callCount] ?? { items: [], total: 3 };
    callCount += 1;
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const collected = [];
  for await (const nft of client.iterateNfts({ pageSize: 1, maxItems: 3 })) {
    collected.push(nft.id);
  }
  assert.deepEqual(collected, ["nft#1", "nft#2", "nft#3"]);
  assert.equal(callCount, 3);
});

test("iterateNftsQuery paginates structured responses", async () => {
  let callCount = 0;
  const fetchImpl = async (url, init) => {
    assert.equal(new URL(url).pathname, "/v1/nfts/query");
    const body = JSON.parse(init.body);
    const offset = Number(body.pagination?.offset ?? 0);
    callCount += 1;
    const items = offset === 0 ? [{ id: "nft#a" }] : [{ id: "nft#b" }];
    return createResponse({
      status: 200,
      jsonData: { items, total: 2 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const ids = [];
  for await (const nft of client.iterateNftsQuery({ pageSize: 1 })) {
    ids.push(nft.id);
  }
  assert.deepEqual(ids, ["nft#a", "nft#b"]);
  assert.equal(callCount, 2);
});

test("iterateNftsQuery enforces maxItems and increments pagination", async () => {
  const seenPagination = [];
  const fetchImpl = async (_url, init) => {
    const envelope = JSON.parse(init.body);
    seenPagination.push(envelope.pagination);
    const offset = Number(envelope.pagination?.offset ?? 0);
    const items =
      offset === 0 ? [{ id: "nft#0" }, { id: "nft#1" }] : [{ id: "nft#2" }];
    return createResponse({
      status: 200,
      jsonData: { items, total: 10 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const ids = [];
  for await (const nft of client.iterateNftsQuery({ pageSize: 2, maxItems: 3 })) {
    ids.push(nft.id);
  }
  assert.deepEqual(ids, ["nft#0", "nft#1", "nft#2"]);
  assert.deepEqual(seenPagination, [{ offset: 0, limit: 2 }, { offset: 2, limit: 1 }]);
});

test("listNfts surfaces permission errors with payload details", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 403,
      statusText: "Forbidden",
      jsonData: { code: "permission_denied", message: "missing role" },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listNfts({ limit: 1 }),
    (error) => {
      assert.ok(error instanceof ToriiHttpError);
      assert.equal(error.status, 403);
      assert.equal(error.code, "permission_denied");
      assert.equal(error.errorMessage, "missing role");
      return true;
    },
  );
});

test("listAccountPermissions encodes pagination and parses response", async () => {
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, accountPath(FIXTURE_ALICE_ID, "/permissions"));
    assert.equal(parsed.searchParams.get("limit"), "5");
    assert.equal(parsed.searchParams.get("offset"), "2");
    return createResponse({
      status: 200,
      jsonData: {
        items: [{ name: "CanMintAsset", payload: { asset: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM" } }],
        total: 1,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.listAccountPermissions(FIXTURE_ALICE_ID, {
    limit: 5,
    offset: 2,
  });
  assert.deepEqual(result, {
    items: [{ name: "CanMintAsset", payload: { asset: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM" } }],
    total: 1,
  });
  await assert.rejects(
    () => client.listAccountPermissions(""),
    /accountId must be a non-empty string/,
  );
});

test("listAccountPermissions rejects non-object options", async () => {
  let fetchCalled = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      fetchCalled = true;
      return createResponse({
        status: 200,
        jsonData: { items: [], total: 0 },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await assert.rejects(
    () => client.listAccountPermissions(FIXTURE_ALICE_ID, 1),
    /listAccountPermissions options must be an object/,
  );
  assert.equal(fetchCalled, false);
});

test("listAccountPermissions rejects invalid signals", async () => {
  let fetchCalled = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      fetchCalled = true;
      return createResponse({
        status: 200,
        jsonData: { items: [], total: 0 },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await assert.rejects(
    () =>
      client.listAccountPermissions(FIXTURE_ALICE_ID, {
        // @ts-expect-error intentional invalid signal for runtime guard
        signal: {},
      }),
    /listAccountPermissions options.signal must be an AbortSignal/,
  );
  assert.equal(fetchCalled, false);
});

test("listAccountPermissions forwards AbortSignal instances", async () => {
  const controller = new AbortController();
  let capturedInit = null;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (_url, init) => {
      capturedInit = init;
      return createResponse({
        status: 200,
        jsonData: { items: [], total: 0 },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await client.listAccountPermissions(FIXTURE_ALICE_ID, {
    limit: 1,
    signal: controller.signal,
  });
  assert.ok(capturedInit);
  assert.strictEqual(capturedInit.signal, controller.signal);
});

test("iterateAccountPermissions paginates account-scoped permissions", async () => {
  let callCount = 0;
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, accountPath(FIXTURE_ALICE_ID, "/permissions"));
    const limit = Number(parsed.searchParams.get("limit"));
    const offset = Number(parsed.searchParams.get("offset") ?? "0");
    callCount += 1;
    let items = [];
    if (offset === 0) {
      items = Array.from({ length: limit }, (_, idx) => ({
        name: `CanMintAsset${idx}`,
        payload: {},
      }));
    } else if (offset === 2) {
      items = [{ name: "CanMintAsset2", payload: {} }];
    }
    return createResponse({
      status: 200,
      jsonData: { items, total: 4 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const collected = [];
  for await (const item of client.iterateAccountPermissions(FIXTURE_ALICE_ID, {
    pageSize: 2,
    maxItems: 3,
  })) {
    collected.push(item.name);
  }
  assert.deepEqual(collected, ["CanMintAsset0", "CanMintAsset1", "CanMintAsset2"]);
  assert.equal(callCount, 2);
});

test("listAccountPermissions validates entry names", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { items: [{ payload: {} }], total: 1 },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.listAccountPermissions(FIXTURE_ALICE_ID),
    /account permission list response\.items\[0]\.name/,
  );
});

test("listAccountPermissions normalizes I105 and i105Default (`sora`) account ids", async () => {
  const forms = sampleAccountForms();
  for (const literal of [forms.i105, forms.i105Default]) {
    let requestedPath = null;
    const fetchImpl = async (url) => {
      requestedPath = new URL(url).pathname;
      return createResponse({
        status: 200,
        jsonData: { items: [], total: 0 },
        headers: { "content-type": "application/json" },
      });
    };
    const client = new ToriiClient(BASE_URL, { fetchImpl });
    await client.listAccountPermissions(literal);
    assert.equal(requestedPath, accountPath(literal, "/permissions"));
  }
});

test("listAccountAssets encodes pagination params", async () => {
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, accountPath(FIXTURE_ALICE_ID, "/assets"));
    assert.equal(parsed.searchParams.get("limit"), "5");
    assert.equal(parsed.searchParams.get("offset"), "1");
    return createResponse({
      status: 200,
      jsonData: {
        items: [{ asset_id: FIXTURE_ASSET_ID_A, quantity: "10" }],
        total: 1,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.listAccountAssets(FIXTURE_ALICE_ID, { limit: 5, offset: 1 });
  assert.equal(payload.items[0].asset_id, FIXTURE_ASSET_ID_A);
});

test("listAccountAssets encodes assetId filters", async () => {
  const assetId = "norito:DEADBEEF";
  const normalizedAssetId = "norito:deadbeef";
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, accountPath(FIXTURE_ALICE_ID, "/assets"));
    assert.equal(parsed.searchParams.get("asset_id"), normalizedAssetId);
    return createResponse({
      status: 200,
      jsonData: { items: [{ asset_id: normalizedAssetId, quantity: "10" }], total: 1 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.listAccountAssets(FIXTURE_ALICE_ID, { assetId });
  assert.equal(payload.items[0].asset_id, normalizedAssetId);
});

test("listAccountAssets rejects unsupported asset#domain#account filters", async () => {
  const invalidAssetId = `62Fk4FPcMuLvW5QjDGNF2a4jAmjM#${FIXTURE_ALICE_ID}`;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not be called");
    },
  });

  await assert.rejects(
    () => client.listAccountAssets(FIXTURE_ALICE_ID, { assetId: invalidAssetId }),
    /must use encoded AssetId form 'norito:<hex>'; legacy 'asset#domain#account' and 'asset##account' forms are not supported/,
  );
});

test("listAccountAssets enforces canonical quantity strings", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          items: [{ asset_id: FIXTURE_ASSET_ID_A, quantity: 10 }],
          total: 1,
        },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.listAccountAssets(FIXTURE_ALICE_ID),
    /account asset list response\.items\[0]\.quantity/,
  );
});

test("listAccountAssets rejects camelCase assetId fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          items: [
            {
              asset_id: FIXTURE_ASSET_ID_A,
              assetId: FIXTURE_ASSET_ID_A,
              quantity: "10",
            },
          ],
          total: 1,
        },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.listAccountAssets(FIXTURE_ALICE_ID),
    /account asset list response\.items\[0]\.assetId is not supported/,
  );
});

test("queryAccountAssets posts structured envelope", async () => {
  let capturedBody;
  const fetchImpl = async (_url, init) => {
    capturedBody = JSON.parse(init.body);
    return createResponse({
      status: 200,
      jsonData: { items: [], total: 0 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.queryAccountAssets(FIXTURE_ALICE_ID, {
    filter: { Gte: ["quantity", 5] },
    sort: [{ key: "quantity", order: "desc" }],
    fetchSize: 10,
  });
  assert.deepEqual(capturedBody.filter, { Gte: ["quantity", 5] });
  assert.deepEqual(capturedBody.sort, [{ key: "quantity", order: "desc" }]);
  assert.equal(capturedBody.fetch_size, 10);
  assert.equal(capturedBody.canonical_i105, undefined);
});

test("queryAccountAssets surfaces errors for invalid filters", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 400,
      jsonData: { code: "ValidationFail", message: "too complex" },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () =>
      client.queryAccountAssets(FIXTURE_ALICE_ID, {
        filter: { IsNull: ["asset_id"] },
      }),
    (error) => {
      assert(error instanceof ToriiHttpError);
      assert.equal(error.status, 400);
      assert.equal(error.code, "ValidationFail");
      return true;
    },
  );
});

test("iterateAccountAssets walks multiple pages", async () => {
  const responses = [
    { items: [{ asset_id: FIXTURE_ASSET_ID_A, quantity: "5" }], total: 2 },
    { items: [{ asset_id: FIXTURE_ASSET_ID_B, quantity: "7" }], total: 2 },
  ];
  let callCount = 0;
  const fetchImpl = async () => {
    const payload = responses[callCount] ?? { items: [], total: 2 };
    callCount += 1;
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const collected = [];
  for await (const holding of client.iterateAccountAssets(FIXTURE_ALICE_ID, { pageSize: 1 })) {
    collected.push(holding.asset_id);
  }
  assert.deepEqual(collected, [FIXTURE_ASSET_ID_A, FIXTURE_ASSET_ID_B]);
});

test("iterateAccountAssetsQuery paginates per-account query endpoint", async () => {
  let callCount = 0;
  const fetchImpl = async (url, init) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, accountPath(FIXTURE_ALICE_ID, "/assets/query"));
    const body = JSON.parse(init.body);
    const offset = Number(body.pagination?.offset ?? 0);
    callCount += 1;
    const items =
      offset === 0
        ? [{ asset_id: FIXTURE_ASSET_ID_A, quantity: "5" }]
        : [{ asset_id: FIXTURE_ASSET_ID_B, quantity: "7" }];
    return createResponse({
      status: 200,
      jsonData: { items, total: 2 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const seen = [];
  for await (const holding of client.iterateAccountAssetsQuery(FIXTURE_ALICE_ID, {
    pageSize: 1,
  })) {
    seen.push(holding.asset_id);
  }
  assert.deepEqual(seen, [FIXTURE_ASSET_ID_A, FIXTURE_ASSET_ID_B]);
  assert.equal(callCount, 2);
});

test("iterateAccountAssets enforces credentials when requirePermissions is set", () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not hit fetch");
    },
  });
  assert.throws(
    () => client.iterateAccountAssets(FIXTURE_ALICE_ID, { requirePermissions: true }),
    /iterateAccountAssets requires authToken or apiToken/,
  );
});

test("iterateAccountAssetsQuery honours requirePermissions with credentials", async () => {
  let callCount = 0;
  const fetchImpl = async () => {
    callCount += 1;
    return createResponse({
      status: 200,
      jsonData: { items: [{ asset_id: FIXTURE_ASSET_ID_A, quantity: "1" }], total: 1 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl, apiToken: "token" });
  const holdings = [];
  for await (const item of client.iterateAccountAssetsQuery(FIXTURE_ALICE_ID, {
    requirePermissions: true,
  })) {
    holdings.push(item.asset_id);
  }
  assert.equal(callCount, 1);
  assert.deepEqual(holdings, [FIXTURE_ASSET_ID_A]);
});

test("iterateAccountAssets enforces maxItems and offset progression", async () => {
  const seenRequests = [];
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    seenRequests.push({
      limit: parsed.searchParams.get("limit"),
      offset: parsed.searchParams.get("offset"),
    });
    const offset = Number(parsed.searchParams.get("offset") ?? "0");
    const page =
      offset === 0
        ? [
            { asset_id: FIXTURE_ASSET_ID_A, quantity: "2" },
            { asset_id: FIXTURE_ASSET_ID_B, quantity: "3" },
          ]
        : [{ asset_id: FIXTURE_ASSET_ID_C, quantity: "5" }];
    return createResponse({
      status: 200,
      jsonData: { items: page, total: 5 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const collected = [];
  for await (const holding of client.iterateAccountAssets(FIXTURE_ALICE_ID, {
    pageSize: 2,
    maxItems: 3,
  })) {
    collected.push(holding.asset_id);
  }
  assert.deepEqual(collected, [FIXTURE_ASSET_ID_A, FIXTURE_ASSET_ID_B, FIXTURE_ASSET_ID_C]);
  assert.deepEqual(seenRequests, [
    { limit: "2", offset: "0" },
    { limit: "1", offset: "2" },
  ]);
});

test("listAccountAssets surfaces permission errors with payload details", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 403,
      statusText: "Forbidden",
      jsonData: { code: "permission_denied", message: "missing permission" },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listAccountAssets(FIXTURE_ALICE_ID, { limit: 1 }),
    (error) => {
      assert.ok(error instanceof ToriiHttpError);
      assert.equal(error.status, 403);
      assert.equal(error.code, "permission_denied");
      assert.equal(error.errorMessage, "missing permission");
      return true;
    },
  );
});

test("queryAccountAssets surfaces permission errors with payload details", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 403,
      statusText: "Forbidden",
      jsonData: { code: "permission_denied", message: "missing role" },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () =>
      client.queryAccountAssets(FIXTURE_ALICE_ID, {
        filter: { Eq: ["asset_id", FIXTURE_ASSET_ID_A] },
      }),
    (error) => {
      assert.ok(error instanceof ToriiHttpError);
      assert.equal(error.status, 403);
      assert.equal(error.code, "permission_denied");
      assert.equal(error.errorMessage, "missing role");
      return true;
    },
  );
});

test("listAccountTransactions encodes pagination params", async () => {
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, accountPath(FIXTURE_ALICE_ID, "/transactions"));
    assert.equal(parsed.searchParams.get("limit"), "3");
    assert.equal(parsed.searchParams.get("offset"), "4");
    return createResponse({
      status: 200,
      jsonData: {
        items: [
          {
            authority: FIXTURE_ALICE_ID,
            entrypoint_hash: "abc",
            result_ok: true,
            timestamp_ms: 123,
          },
        ],
        total: 1,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.listAccountTransactions(FIXTURE_ALICE_ID, {
    limit: 3,
    offset: 4,
  });
  assert.equal(payload.items[0].entrypoint_hash, "abc");
});

test("listAccountTransactions encodes assetId filters", async () => {
  const assetId = "norito:DEADBEEF";
  const normalizedAssetId = "norito:deadbeef";
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, accountPath(FIXTURE_ALICE_ID, "/transactions"));
    assert.equal(parsed.searchParams.get("asset_id"), normalizedAssetId);
    return createResponse({
      status: 200,
      jsonData: {
        items: [{ entrypoint_hash: "abc", result_ok: true }],
        total: 1,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.listAccountTransactions(FIXTURE_ALICE_ID, {
    assetId,
  });
  assert.equal(payload.items[0].entrypoint_hash, "abc");
});

test("listAccountTransactions validates boolean result fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          items: [{ entrypoint_hash: "tx1", result_ok: "maybe" }],
          total: 1,
        },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.listAccountTransactions(FIXTURE_ALICE_ID),
    /account transaction list response\.items\[0]\.result_ok/,
  );
});

test("listAccountTransactions rejects camelCase entrypointHash fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          items: [
            {
              entrypoint_hash: "tx1",
              entrypointHash: "tx1",
              result_ok: true,
            },
          ],
          total: 1,
        },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.listAccountTransactions(FIXTURE_ALICE_ID),
    /account transaction list response\.items\[0]\.entrypointHash is not supported/,
  );
});

test("queryAccountTransactions posts structured envelope", async () => {
  let capturedBody;
  const fetchImpl = async (_url, init) => {
    assert.equal(init.method, "POST");
    capturedBody = JSON.parse(init.body);
    return createResponse({
      status: 200,
      jsonData: { items: [], total: 0 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.queryAccountTransactions(FIXTURE_ALICE_ID, {
    filter: { Eq: ["authority", FIXTURE_ALICE_ID] },
    sort: [{ key: "timestamp_ms", order: "desc" }],
    fetchSize: 5,
    queryName: "AccountTransactions",
  });
  assert.deepEqual(capturedBody.filter, { Eq: ["authority", FIXTURE_ALICE_ID] });
  assert.deepEqual(capturedBody.sort, [{ key: "timestamp_ms", order: "desc" }]);
  assert.equal(capturedBody.fetch_size, 5);
  assert.equal(capturedBody.query, "AccountTransactions");
});

test("iterateAccountTransactions paginates results", async () => {
  const responses = [
    {
      items: [{ entrypoint_hash: "tx1", result_ok: true }],
      total: 3,
    },
    {
      items: [{ entrypoint_hash: "tx2", result_ok: false }],
      total: 3,
    },
    {
      items: [{ entrypoint_hash: "tx3", result_ok: true }],
      total: 3,
    },
  ];
  let callCount = 0;
  const fetchImpl = async () => {
    const payload = responses[callCount] ?? { items: [], total: 3 };
    callCount += 1;
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const hashes = [];
  for await (const tx of client.iterateAccountTransactions(FIXTURE_ALICE_ID, {
    pageSize: 1,
    maxItems: 3,
  })) {
    hashes.push(tx.entrypoint_hash);
  }
  assert.deepEqual(hashes, ["tx1", "tx2", "tx3"]);
  assert.equal(callCount, 3);
});

test("iterateAccountTransactionsQuery walks query endpoint", async () => {
  let callCount = 0;
  const fetchImpl = async (url, init) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, accountPath(FIXTURE_ALICE_ID, "/transactions/query"));
    const body = JSON.parse(init.body);
    const offset = Number(body.pagination?.offset ?? 0);
    callCount += 1;
    const items =
      offset === 0
        ? [{ entrypoint_hash: "tx1", result_ok: true }]
        : [{ entrypoint_hash: "tx2", result_ok: false }];
    return createResponse({
      status: 200,
      jsonData: { items, total: 2 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const hashes = [];
  for await (const tx of client.iterateAccountTransactionsQuery(FIXTURE_ALICE_ID, {
    pageSize: 1,
  })) {
    hashes.push(tx.entrypoint_hash);
  }
  assert.deepEqual(hashes, ["tx1", "tx2"]);
  assert.equal(callCount, 2);
});

test("listAccountAssets rejects blank account ids", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({ status: 200, jsonData: { items: [], total: 0 }, headers: { "content-type": "application/json" } }),
  });
  await assert.rejects(
    () => client.listAccountAssets("", {}),
    /accountId must be a non-empty string/,
  );
});

test("listAccountAssets trims and encodes path segments", async () => {
  let capturedPath;
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    capturedPath = parsed.pathname;
    return createResponse({
      status: 200,
      jsonData: { items: [], total: 0 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.listAccountAssets(`  ${FIXTURE_ALICE_ID}  `);
  assert.equal(capturedPath, accountPath(FIXTURE_ALICE_ID, "/assets"));
});

test("listAssetHolders encodes definition id", async () => {
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/assets/62Fk4FPcMuLvW5QjDGNF2a4jAmjM/holders");
    return createResponse({
      status: 200,
      jsonData: {
        items: [{ account_id: FIXTURE_ALICE_ID, quantity: "10" }],
        total: 1,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.listAssetHolders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
  assert.equal(payload.items[0].account_id, FIXTURE_ALICE_ID);
});

test("listAssetHolders encodes assetId filters", async () => {
  const assetId = "norito:DEADBEEF";
  const normalizedAssetId = "norito:deadbeef";
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/assets/62Fk4FPcMuLvW5QjDGNF2a4jAmjM/holders");
    assert.equal(parsed.searchParams.get("asset_id"), normalizedAssetId);
    return createResponse({
      status: 200,
      jsonData: {
        items: [{ account_id: FIXTURE_ALICE_ID, quantity: "5" }],
        total: 1,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.listAssetHolders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", { assetId });
  assert.equal(payload.items[0].account_id, FIXTURE_ALICE_ID);
});

test("listAssetHolders validates holder identifiers", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { items: [{ quantity: "5" }], total: 1 },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.listAssetHolders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
    /asset holder list response\.items\[0]\.account_id/,
  );
});

test("listAssetHolders rejects camelCase accountId fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          items: [
            {
              account_id: FIXTURE_ALICE_ID,
              accountId: FIXTURE_ALICE_ID,
              quantity: "10",
            },
          ],
          total: 1,
        },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.listAssetHolders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
    /asset holder list response\.items\[0]\.accountId is not supported/,
  );
});

test("queryAssetHolders posts encoded definition path", async () => {
  const fetchImpl = async (url, init) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/assets/62Fk4FPcMuLvW5QjDGNF2a4jAmjM/holders/query");
    assert.equal(init.method, "POST");
    return createResponse({
      status: 200,
      jsonData: { items: [], total: 0 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.queryAssetHolders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", {});
});

test("iterateAssetHolders paginates holder list", async () => {
  const responses = [
    { items: [{ account_id: FIXTURE_ALICE_ID, quantity: "5" }], total: 2 },
    { items: [{ account_id: FIXTURE_BOB_NARNIA_ID, quantity: "4" }], total: 2 },
  ];
  let callCount = 0;
  const fetchImpl = async () => {
    const payload = responses[callCount] ?? { items: [], total: 2 };
    callCount += 1;
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const holders = [];
  for await (const holder of client.iterateAssetHolders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", { pageSize: 1 })) {
    holders.push(holder.account_id);
  }
  assert.deepEqual(holders, [FIXTURE_ALICE_ID, FIXTURE_BOB_NARNIA_ID]);
});

test("iterateAssetHoldersQuery paginates query responses", async () => {
  let callCount = 0;
  const fetchImpl = async (url, init) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/assets/62Fk4FPcMuLvW5QjDGNF2a4jAmjM/holders/query");
    const body = JSON.parse(init.body);
    const offset = Number(body.pagination?.offset ?? 0);
    callCount += 1;
    const items =
      offset === 0
        ? [{ account_id: FIXTURE_ALICE_ID, quantity: "5" }]
        : [{ account_id: FIXTURE_BOB_NARNIA_ID, quantity: "4" }];
    return createResponse({
      status: 200,
      jsonData: { items, total: 2 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const ids = [];
  for await (const holder of client.iterateAssetHoldersQuery("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", {
    pageSize: 1,
  })) {
    ids.push(holder.account_id);
  }
  assert.deepEqual(ids, [FIXTURE_ALICE_ID, FIXTURE_BOB_NARNIA_ID]);
  assert.equal(callCount, 2);
});

test("iterateContractInstances paginates registry results", async () => {
  const responses = [
    {
      namespace: "apps",
      total: 3,
      offset: 0,
      limit: 2,
      instances: [
        { contract_id: "calc.v1", code_hash_hex: fakeHashHex(0xaa) },
        { contract_id: "mint.v1", code_hash_hex: fakeHashHex(0xbb) },
      ],
    },
    {
      namespace: "apps",
      total: 3,
      offset: 2,
      limit: 2,
      instances: [{ contract_id: "vault.v1", code_hash_hex: fakeHashHex(0xcc) }],
    },
  ];
  let callCount = 0;
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/contracts/instances/apps");
    assert.equal(parsed.searchParams.get("contains"), "calc");
    assert.equal(parsed.searchParams.get("limit"), "2");
    assert.equal(parsed.searchParams.get("offset"), String(callCount * 2));
    const payload = responses[callCount] ?? {
      namespace: "apps",
      total: 3,
      offset: parsed.searchParams.get("offset") ?? "0",
      limit: 2,
      instances: [],
    };
    callCount += 1;
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const ids = [];
  for await (const instance of client.iterateContractInstances("apps", {
    contains: "calc",
    pageSize: 2,
  })) {
    ids.push(instance.contract_id);
  }
  assert.deepEqual(ids, ["calc.v1", "mint.v1", "vault.v1"]);
  assert.equal(callCount, 2);
});

test("iterateGovernanceInstances honours maxItems", async () => {
  const responses = [
    {
      namespace: "apps",
      total: 4,
      offset: 0,
      limit: 2,
      instances: [
        { contract_id: "calc.v1", code_hash_hex: fakeHashHex(0xaa) },
        { contract_id: "mint.v1", code_hash_hex: fakeHashHex(0xbb) },
      ],
    },
    {
      namespace: "apps",
      total: 4,
      offset: 2,
      limit: 2,
      instances: [
        { contract_id: "vault.v1", code_hash_hex: fakeHashHex(0xcc) },
        { contract_id: "audit.v1", code_hash_hex: fakeHashHex(0xdd) },
      ],
    },
  ];
  let callCount = 0;
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/gov/instances/apps");
    assert.equal(parsed.searchParams.get("hash_prefix"), "abcd");
    const payload = responses[callCount] ?? {
      namespace: "apps",
      total: 4,
      offset: parsed.searchParams.get("offset") ?? "0",
      limit: 2,
      instances: [],
    };
    callCount += 1;
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const ids = [];
  for await (const instance of client.iterateGovernanceInstances("apps", {
    hashPrefix: "abcd",
    pageSize: 2,
    maxItems: 3,
  })) {
    ids.push(instance.contract_id);
  }
  assert.deepEqual(ids, ["calc.v1", "mint.v1", "vault.v1"]);
  assert.equal(callCount, 2);
});

test("iterateTriggers paginates list endpoint", async () => {
  let callCount = 0;
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/triggers");
    assert.equal(parsed.searchParams.get("namespace"), "apps");
    const offset = Number(parsed.searchParams.get("offset") ?? "0");
    callCount += 1;
    const items =
      offset === 0
        ? [
            { id: "trigger-1", owner: FIXTURE_ALICE_ID },
            { id: "trigger-2", owner: FIXTURE_BOB_ID },
          ]
        : [{ id: "trigger-3", owner: FIXTURE_CAROL_ID }];
    return createResponse({
      status: 200,
      jsonData: { items, total: 3 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const ids = [];
  for await (const trigger of client.iterateTriggers({ namespace: "apps", pageSize: 2 })) {
    ids.push(trigger.id);
  }
  assert.deepEqual(ids, ["trigger-1", "trigger-2", "trigger-3"]);
  assert.equal(callCount, 2);
});

test("iterateTriggersQuery paginates query payloads", async () => {
  let callCount = 0;
  const fetchImpl = async (url, init) => {
    assert.equal(init.method, "POST");
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/triggers/query");
    const body = JSON.parse(init.body);
    const offset = Number(body.pagination?.offset ?? 0);
    assert.deepEqual(body.filter, { Eq: ["object.authority", FIXTURE_ALICE_ID] });
    callCount += 1;
    const items =
      offset === 0
        ? [
            { id: "trigger-1", owner: FIXTURE_ALICE_ID },
            { id: "trigger-2", owner: FIXTURE_ALICE_ID },
          ]
        : [{ id: "trigger-3", owner: FIXTURE_ALICE_ID }];
    return createResponse({
      status: 200,
      jsonData: { items, total: 3 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const ids = [];
  for await (const trigger of client.iterateTriggersQuery({
    filter: { Eq: ["object.authority", FIXTURE_ALICE_ID] },
    pageSize: 2,
  })) {
    ids.push(trigger.id);
  }
  assert.deepEqual(ids, ["trigger-1", "trigger-2", "trigger-3"]);
  assert.equal(callCount, 2);
});

test("resolveToriiClientConfig merges config, env, and overrides", async () => {
  await withEnv(
    {
      IROHA_TORII_TIMEOUT_MS: "1500",
      IROHA_TORII_RETRY_STATUSES: "500, 503",
    },
    () => {
      const resolved = resolveToriiClientConfig({
        config: {
          torii: { apiTokens: ["from-config"] },
          toriiClient: {
            maxRetries: 4,
            retryMethods: ["get", "post"],
            retryProfiles: {
              streaming: { maxRetries: 9 },
            },
          },
        },
        overrides: {
          apiToken: "override-token",
          backoffInitialMs: 250,
          retryProfiles: {
            pipeline: { maxRetries: 6, retryMethods: ["post"] },
          },
        },
      });
      assert.equal(resolved.timeoutMs, 1500);
      assert.equal(resolved.maxRetries, 4);
      assert.equal(resolved.backoffInitialMs, 250);
      assert.ok(resolved.retryMethods.has("POST"));
      assert.ok(resolved.retryStatuses.has(500));
      assert.equal(resolved.apiToken, "override-token");
      assert.deepEqual(resolved.retryProfiles.pipeline.maxRetries, 6);
      assert.ok(resolved.retryProfiles.pipeline.retryMethods.has("POST"));
      assert.equal(resolved.retryProfiles.streaming.maxRetries, 9);
    },
  );
});

test("extractToriiFeatureConfig normalizes feature sections", () => {
  const hashedAccountRaw = SAMPLE_ACCOUNT_FORMS.nonCanonicalI105;
  const hashedAccountCanonical = normalizeAccountId(
    hashedAccountRaw,
    "hashedAccount",
  );
  const snapshot = extractToriiFeatureConfig({
    config: {
      torii: {
        iso_bridge: {
          enabled: true,
          dedupe_ttl_secs: 30,
          signer: { account_id: hashedAccountRaw, private_key: "ed01" },
          account_aliases: [
            { iban: VALID_IBAN, account_id: hashedAccountRaw },
            { iban: "DE89370400440532013000", account_id: FIXTURE_ALICE_TEST_ID },
          ],
          currency_assets: [{ currency: "USD", asset_definition: "usd#bank" }],
        },
        rbc_sampling: {
          enabled: true,
          max_samples_per_request: 4,
          max_bytes_per_request: 1024,
          daily_byte_budget: 2048,
          rate_per_minute: 12,
        },
      },
      connect: {
        enabled: false,
        ws_max_sessions: 10,
        ws_per_ip_max_sessions: 2,
        ws_rate_per_ip_per_min: 60,
        session_ttl_ms: 1000,
        frame_max_bytes: 1024,
        session_buffer_max_bytes: 2048,
        dedupe_ttl_ms: 500,
        dedupe_cap: 16,
        relay_enabled: true,
        relay_strategy: "broadcast",
        p2p_ttl_hops: 1,
      },
    },
  });
  assert.ok(snapshot.isoBridge?.enabled);
  const signerAccountId = snapshot.isoBridge?.signer?.accountId
    ? normalizeAccountId(snapshot.isoBridge.signer.accountId, "isoBridge.signer.accountId")
    : null;
  assert.equal(signerAccountId, hashedAccountCanonical);
  assert.equal(snapshot.isoBridge?.accountAliases.length, 2);
  const aliasAccountId = snapshot.isoBridge?.accountAliases[0]?.accountId
    ? normalizeAccountId(
        snapshot.isoBridge.accountAliases[0].accountId,
        "isoBridge.accountAliases[0].accountId",
      )
    : null;
  assert.equal(aliasAccountId, hashedAccountCanonical);
  assert.equal(snapshot.isoBridge?.accountAliases[1]?.accountId, FIXTURE_ALICE_TEST_ID);
  assert.equal(snapshot.rbcSampling?.maxSamplesPerRequest, 4);
  assert.equal(snapshot.connect?.wsMaxSessions, 10);
});

test("extractConfidentialGasConfig returns normalized schedule", () => {
  const config = {
    confidential: {
      gas: {
        proof_base: 250_000,
        per_public_input: 2_000,
        per_proof_byte: 5,
        per_nullifier: 300,
        per_commitment: 500,
      },
    },
  };
  const gas = extractConfidentialGasConfig({ config });
  assert.deepEqual(gas, {
    proofBase: 250_000,
    perPublicInput: 2_000,
    perProofByte: 5,
    perNullifier: 300,
    perCommitment: 500,
  });
});

test("ToriiClient.getConfidentialGasSchedule fetches schedule", async () => {
  const payload = {
    confidential_gas: {
      proof_base: 111,
      per_public_input: 22,
      per_proof_byte: 3,
      per_nullifier: 4,
      per_commitment: 5,
    },
  };
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const gas = await client.getConfidentialGasSchedule();
  assert.deepEqual(gas, {
    proofBase: 111,
    perPublicInput: 22,
    perProofByte: 3,
    perNullifier: 4,
    perCommitment: 5,
  });
});

test("ToriiClient.getConfigurationTyped normalizes snapshot", async () => {
  const payload = {
    public_key: "ed01",
    logger: {
      level: "info",
      filter: "torii=debug",
    },
    network: {
      block_gossip_size: 512,
      block_gossip_period_ms: 250,
      transaction_gossip_size: 64,
      transaction_gossip_period_ms: 150,
    },
    queue: {
      capacity: 4096,
    },
    confidential_gas: {
      proof_base: 100,
      per_public_input: 10,
      per_proof_byte: 1,
      per_nullifier: 2,
      per_commitment: 3,
    },
    transport: {
      norito_rpc: {
        enabled: true,
        stage: "ga",
        require_mtls: true,
        canary_allowlist_size: 3,
      },
      streaming: {
        soranet: {
          enabled: true,
          stream_tag: "norito-stream",
          exit_multiaddr: "/dns/exit/udp/9443/quic",
          padding_budget_ms: 10,
          access_kind: "read-only",
          gar_category: "stream.norito.read_only",
          channel_salt: "test-salt",
          provision_spool_dir: "./storage/streaming/soranet_routes",
          provision_window_segments: 4,
          provision_queue_capacity: 128,
        },
      },
    },
    nexus: {
      axt: {
        slot_length_ms: 1_000,
        max_clock_skew_ms: 250,
        proof_cache_ttl_slots: 3,
        replay_retention_slots: 256,
      },
    },
  };
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const snapshot = await client.getConfigurationTyped();
  assert.deepEqual(snapshot, {
    publicKeyHex: "ed01",
    logger: { level: "info", filter: "torii=debug" },
    network: {
      blockGossipSize: 512,
      blockGossipPeriodMs: 250,
      transactionGossipSize: 64,
      transactionGossipPeriodMs: 150,
    },
    queue: { capacity: 4096 },
    confidentialGas: {
      proofBase: 100,
      perPublicInput: 10,
      perProofByte: 1,
      perNullifier: 2,
      perCommitment: 3,
    },
    transport: {
      noritoRpc: {
        enabled: true,
        stage: "ga",
        requireMtls: true,
        canaryAllowlistSize: 3,
      },
      streaming: {
        soranet: {
          enabled: true,
          streamTag: "norito-stream",
          exitMultiaddr: "/dns/exit/udp/9443/quic",
          paddingBudgetMs: 10,
          accessKind: "read-only",
          garCategory: "stream.norito.read_only",
          channelSalt: "test-salt",
          provisionSpoolDir: "./storage/streaming/soranet_routes",
          provisionWindowSegments: 4,
          provisionQueueCapacity: 128,
        },
      },
    },
    nexus: {
      axt: {
        slotLengthMs: 1_000,
        maxClockSkewMs: 250,
        proofCacheTtlSlots: 3,
        replayRetentionSlots: 256,
      },
    },
  });
});

test("ToriiClient.getConfigurationTyped returns null on 404", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 404,
      jsonData: null,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const snapshot = await client.getConfigurationTyped();
  assert.strictEqual(snapshot, null);
});

test("ToriiClient applies default headers and tokens", async () => {
  await withEnv(
    {
      IROHA_TORII_API_TOKEN: "env-token",
    },
    async () => {
      const captures = [];
      const fetchImpl = async (url, init) => {
        captures.push({ url, init });
        return createResponse({
          status: 200,
          jsonData: [],
          headers: { "content-type": "application/json" },
        });
      };
      const client = new ToriiClient(BASE_URL, {
        fetchImpl,
        defaultHeaders: { "User-Agent": "iroha-js" },
        authToken: "local-auth",
        allowInsecure: true,
      });
      await client.listAttachments();
      assert.equal(captures.length, 1);
      const headers = captures[0].init.headers;
      assert.equal(headers["User-Agent"], "iroha-js");
      assert.equal(headers["X-API-Token"], "env-token");
      assert.equal(headers["X-Iroha-API-Token"], undefined);
      assert.equal(headers.Authorization, "Bearer local-auth");
    },
  );
});

test("ToriiClient retries retryable statuses", async () => {
  let attempt = 0;
  const fetchImpl = async () => {
    attempt += 1;
    if (attempt < 3) {
      return createResponse({ status: 503, jsonData: {} });
    }
    return createResponse({
      status: 200,
      jsonData: { status: "OK" },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, {
    fetchImpl,
    maxRetries: 3,
    backoffInitialMs: 0,
  });
  const response = await client.getHealth();
  assert.deepEqual(response, { status: "OK" });
  assert.equal(attempt, 3);
});

test("ToriiClient emits retry telemetry events", async () => {
  let attempt = 0;
  const events = [];
  const fetchImpl = async () => {
    attempt += 1;
    if (attempt === 1) {
      return createResponse({ status: 503, jsonData: {} });
    }
    return createResponse({
      status: 200,
      jsonData: { status: "OK" },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, {
    fetchImpl,
    maxRetries: 2,
    backoffInitialMs: 0,
    retryTelemetryHook: (event) => events.push(event),
  });
  const response = await client.getHealth();
  assert.equal(response?.status, "OK");
  assert.equal(attempt, 2);
  assert.equal(events.length, 1);
  const event = events[0];
  assert.equal(event.phase, "response");
  assert.equal(event.attempt, 1);
  assert.equal(event.nextAttempt, 2);
  assert.equal(event.maxRetries, 2);
  assert.equal(event.method, "GET");
  assert.equal(event.status, 503);
  assert.equal(typeof event.timestampMs, "number");
  assert.equal(typeof event.durationMs, "number");
  assert.ok(event.durationMs >= 0);
});

test("ToriiClient enforces request timeout", async () => {
  const fetchImpl = async (_url, init) =>
    new Promise((_, reject) => {
      init.signal?.addEventListener(
        "abort",
        () => {
          const abortError =
            typeof DOMException !== "undefined"
              ? new DOMException("Aborted", "AbortError")
              : Object.assign(new Error("Aborted"), { name: "AbortError" });
          reject(abortError);
        },
        { once: true },
      );
    });
  const client = new ToriiClient(BASE_URL, {
    fetchImpl,
    timeoutMs: 10,
    maxRetries: 0,
  });
  await assert.rejects(
    () => client.listAttachments(),
    /AbortError|aborted/i,
  );
});

test("streamEvents yields parsed SSE payloads", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(
      url,
      `${BASE_URL}/v1/events/sse?filter=${encodeURIComponent('{"Pipeline":{"Block":{}}}')}`,
    );
    assert.equal(init.headers["Last-Event-ID"], "resume-id");
    return createSseResponse([
      "id: block-1\n",
      "event: pipeline.block\n",
      'data: {"height":1}\n',
      "\n",
    ]);
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const iterator = client.streamEvents({
    filter: { Pipeline: { Block: {} } },
    lastEventId: "resume-id",
  });
  const first = await iterator.next();
  assert.equal(first.done, false);
  assert.deepEqual(first.value, {
    event: "pipeline.block",
    data: { height: 1 },
    id: "block-1",
    retry: null,
    raw: '{"height":1}',
  });
  const second = await iterator.next();
  assert.equal(second.done, true);
});

test("streamEvents retries SSE handshake using streaming profile", async () => {
  let attempts = 0;
  const fetchImpl = async (_url) => {
    attempts += 1;
    if (attempts === 1) {
      return createResponse({ status: 503, jsonData: {} });
    }
    return createSseResponse([
      "id: block-2\n",
      "event: pipeline.block\n",
      'data: {"height":2}\n',
      "\n",
    ]);
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl, maxRetries: 0 });
  const iterator = client.streamEvents();
  const first = await iterator.next();
  assert.equal(first.done, false);
  assert.equal(first.value?.id, "block-2");
  assert.equal(attempts, 2);
});

test("streamSumeragiStatus streams SSE without filters", async () => {
  let requestHeaders;
  const fetchImpl = async (url, init) => {
    requestHeaders = init.headers;
    assert.equal(url, `${BASE_URL}/v1/sumeragi/status/sse`);
    return createSseResponse([
      "event: sumeragi.status\n",
      'data: {"view":2}\n',
      "\n",
    ]);
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const iterator = client.streamSumeragiStatus();
  const first = await iterator.next();
  assert.equal(first.done, false);
  assert.deepEqual(first.value, {
    event: "sumeragi.status",
    data: { view: 2 },
    id: null,
    retry: null,
    raw: '{"view":2}',
  });
  const next = await iterator.next();
  assert.equal(next.done, true);
  assert.equal(requestHeaders.Accept, "text/event-stream");
});

test("streamEvents rejects unsupported filter types", () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  assert.throws(
    () => client.streamEvents({ filter: 42 }),
    /string or plain object/,
  );
});

test("streamEvents enforces option shapes", () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  assert.throws(
    () => client.streamEvents("invalid"),
    /streamEvents options must be an object/,
  );
  assert.throws(
    () => client.streamEvents({ lastEventId: "" }),
    /streamEvents\.lastEventId must not be empty/,
  );
});

test("streamEvents rejects unsupported options", () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  assert.throws(
    () => client.streamEvents({ filter: {}, extra: true }),
    /streamEvents options contains unsupported fields: extra/,
  );
});

test("streamSumeragiStatus rejects unsupported options", () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  assert.throws(
    () => client.streamSumeragiStatus({ unexpected: "oops" }),
    /streamSumeragiStatus options contains unsupported fields: unexpected/,
  );
});

test("streamKaigiRelayEvents enforces lastEventId strings", () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  assert.throws(
    () => client.streamKaigiRelayEvents({ lastEventId: 42 }),
    /streamKaigiRelayEvents\.lastEventId must be a string/,
  );
});

test("streamKaigiRelayEvents rejects unsupported options", () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  assert.throws(
    () => client.streamKaigiRelayEvents({ domain: "kaigi", extra: true }),
    /streamKaigiRelayEvents options contains unsupported fields: extra/,
  );
});

test("listKaigiRelays rejects unsupported option keys", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listKaigiRelays({ extra: true }),
    /listKaigiRelays options contains unsupported fields: extra/,
  );
});

test("listKaigiRelays normalizes summary payloads", async () => {
  let requested;
  const fetchImpl = async (url) => {
    requested = url;
    return createResponse({
      status: 200,
      jsonData: {
        total: "2",
        items: [
          {
            relay_id: "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw",
            domain: "kaigi",
            bandwidth_class: 5,
            hpke_fingerprint_hex: "aa".repeat(32),
            status: "Healthy",
            reported_at_ms: "42",
          },
        ],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.listKaigiRelays();
  assert.equal(requested, `${BASE_URL}/v1/kaigi/relays`);
  assert.equal(payload.total, 2);
  assert.equal(payload.items.length, 1);
  assert.equal(payload.items[0].status, "healthy");
  assert.equal(payload.items[0].reported_at_ms, 42);
});

test("listKaigiRelays forwards AbortSignal", async () => {
  const controller = new AbortController();
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/kaigi/relays`);
    assert.strictEqual(init.signal, controller.signal);
    return createResponse({
      status: 200,
      jsonData: { total: 0, items: [] },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.listKaigiRelays({ signal: controller.signal });
  assert.equal(payload.total, 0);
});
test("getKaigiRelay returns null on 404 and normalizes detail response", async () => {
  const relayId = FIXTURE_ALICE_ID;
  const fallbackClient = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 404 }),
  });
  const missing = await fallbackClient.getKaigiRelay(relayId);
  assert.equal(missing, null);

  let requested;
  const fetchImpl = async (url) => {
    requested = url;
    return createResponse({
      status: 200,
      jsonData: {
        relay: {
          relay_id: relayId,
          domain: "kaigi",
          bandwidth_class: 7,
          hpke_fingerprint_hex: "bb".repeat(32),
          status: "degraded",
          reported_at_ms: 99,
        },
        hpke_public_key_b64: "qrvM",
        reported_call: { domain_id: "kaigi", call_name: "demo" },
        reported_by: "ops@kaigi",
        notes: "staged",
        metrics: {
          domain: "kaigi",
          registrations_total: 3,
          manifest_updates_total: 1,
          failovers_total: 2,
          health_reports_total: 4,
        },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const detail = await client.getKaigiRelay(relayId);
  assert.equal(requested, `${BASE_URL}/v1/kaigi/relays/${encodeURIComponent(relayId)}`);
  assert.equal(detail?.hpke_public_key_b64, "qrvM");
  assert.equal(detail?.reported_call?.call_name, "demo");
  assert.equal(detail?.metrics?.registrations_total, 3);
});

test("getKaigiRelay rejects unsupported option keys", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getKaigiRelay(SAMPLE_ACCOUNT_FORMS.i105, { extra: true }),
    /getKaigiRelay options contains unsupported fields: extra/,
  );
});

test("getKaigiRelay forwards AbortSignal", async () => {
  const relayId = FIXTURE_BOB_ID;
  const controller = new AbortController();
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/kaigi/relays/${encodeURIComponent(relayId)}`);
    assert.strictEqual(init.signal, controller.signal);
    return createResponse({
      status: 200,
      jsonData: {
        relay: {
          relay_id: relayId,
          domain: "kaigi",
          bandwidth_class: 1,
          hpke_fingerprint_hex: "aa".repeat(32),
          status: "healthy",
          reported_at_ms: 1,
          hpke_public_key_b64: "qrvM",
        },
        hpke_public_key_b64: "qrvM",
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const detail = await client.getKaigiRelay(relayId, { signal: controller.signal });
  assert.equal(detail?.relay?.relay_id, relayId);
});

test("getKaigiRelaysHealth parses counters and domain metrics", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        healthy_total: 2,
        degraded_total: 1,
        unavailable_total: 0,
        reports_total: 5,
        registrations_total: 3,
        failovers_total: 1,
        domains: [
          {
            domain: "kaigi",
            registrations_total: 3,
            manifest_updates_total: 2,
            failovers_total: 1,
            health_reports_total: 5,
          },
        ],
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const snapshot = await client.getKaigiRelaysHealth();
  assert.equal(snapshot.healthy_total, 2);
  assert.equal(snapshot.domains[0].manifest_updates_total, 2);
});

test("getKaigiRelaysHealth rejects unsupported option keys", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getKaigiRelaysHealth({ extra: true }),
    /getKaigiRelaysHealth options contains unsupported fields: extra/,
  );
});

test("getKaigiRelaysHealth forwards AbortSignal", async () => {
  const controller = new AbortController();
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/kaigi/relays/health`);
    assert.strictEqual(init.signal, controller.signal);
    return createResponse({
      status: 200,
      jsonData: {
        healthy_total: 0,
        degraded_total: 0,
        unavailable_total: 0,
        reports_total: 0,
        registrations_total: 0,
        failovers_total: 0,
        domains: [],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const snapshot = await client.getKaigiRelaysHealth({ signal: controller.signal });
  assert.equal(snapshot.healthy_total, 0);
});

test("streamKaigiRelayEvents encodes filters and normalizes payloads", async () => {
  let requested;
  const fetchImpl = async (url, init) => {
    requested = url;
    assert.equal(init.headers["Last-Event-ID"], "cursor");
    return createSseResponse([
      'event: kaigi\n',
      `data: {"kind":"registration","domain":"kaigi","relay_id":"6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw","bandwidth_class":1,"hpke_fingerprint_hex":"${"aa".repeat(32)}"}\n`,
      "\n",
      'event: kaigi\n',
      'data: {"kind":"health","domain":"kaigi","relay_id":"6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw","status":"degraded","reported_at_ms":5000,"call":{"domain":"kaigi","name":"demo"}}\n',
      "\n",
    ]);
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const iterator = client.streamKaigiRelayEvents({
    domain: "Kaigi",
    relay: "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw",
    kind: ["registration", "health"],
    lastEventId: "cursor",
  });
  const first = await iterator.next();
  assert.equal(first.value?.data?.kind, "registration");
  const second = await iterator.next();
  assert.equal(second.value?.data?.status, "degraded");
  assert.equal(second.value?.data?.call.name, "demo");
  assert.ok(requested?.includes("domain=kaigi"));
  assert.ok(
    requested?.includes("relay=6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw"),
  );
  assert.ok(requested?.includes("kind=registration%2Chealth"));
});

test("listProverReports encodes filters", async () => {
  const fetchImpl = async (url) => {
    assert.ok(url.includes("ok_only=true"));
    assert.ok(url.includes("limit=5"));
    return createResponse({
      status: 200,
      jsonData: [
        {
          id: "r-1",
          ok: false,
          error: "decode failed",
          content_type: "application/json",
          size: 10,
          created_ms: 1,
          processed_ms: 2,
          latency_ms: 1,
          zk1_tags: ["PROF"],
        },
      ],
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.listProverReports({ ok_only: true, limit: 5, ignored: null });
  assert.deepEqual(result, {
    kind: "reports",
    reports: [
      {
        id: "r-1",
        ok: false,
        error: "decode failed",
        content_type: "application/json",
        size: 10,
        created_ms: 1,
        processed_ms: 2,
        latency_ms: 1,
        zk1_tags: ["PROF"],
      },
    ],
  });
});

test("listProverReports rejects ids_only projections without filter", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: ["rep-1"],
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.listProverReports({}),
    /ids_only/,
  );
});

test("listProverReports returns ids when ids_only flag set", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: ["rep-1", "rep-2"],
        headers: { "content-type": "application/json" },
      }),
  });
  const result = await client.listProverReports({ ids_only: true });
  assert.deepEqual(result, { kind: "ids", ids: ["rep-1", "rep-2"] });
});

test("listProverReports returns message summaries when messages_only flag set", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: [
          { id: "rep-1", error: "oops" },
          { id: "rep-2", error: null },
        ],
        headers: { "content-type": "application/json" },
      }),
  });
  const result = await client.listProverReports({ messagesOnly: true });
  assert.deepEqual(result, {
    kind: "messages",
    messages: [
      { id: "rep-1", error: "oops" },
      { id: "rep-2", error: null },
    ],
  });
});

test("listProverReports rejects messages_only projection without filter", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: [{ id: "rep-1", error: "oops" }],
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(() => client.listProverReports({}), /messages_only/);
});

test("listProverReports normalizes prover filter inputs", async () => {
  let requestedUrl = "";
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url) => {
      requestedUrl = url;
      return createResponse({
        status: 200,
        jsonData: [],
        headers: { "content-type": "application/json" },
      });
    },
  });
  await client.listProverReports({
    okOnly: true,
    contentType: " application/json ",
    hasTag: "PROF",
    limit: "10",
    offset: 4n,
    sinceMs: "123",
    beforeMs: 456n,
    idsOnly: true,
    latest: true,
    order: "DESC",
    id: "rep-42",
  });
  assert.ok(requestedUrl.includes("/v1/zk/prover/reports?"));
  const params = new URL(requestedUrl).searchParams;
  assert.equal(params.get("ok_only"), "true");
  assert.equal(params.get("content_type"), "application/json");
  assert.equal(params.get("has_tag"), "PROF");
  assert.equal(params.get("limit"), "10");
  assert.equal(params.get("offset"), "4");
  assert.equal(params.get("since_ms"), "123");
  assert.equal(params.get("before_ms"), "456");
  assert.equal(params.get("ids_only"), "true");
  assert.equal(params.get("latest"), "true");
  assert.equal(params.get("order"), "desc");
  assert.equal(params.get("id"), "rep-42");
});

test("prover filter validation rejects invalid entries", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("unexpected fetch");
    },
  });
  await assert.rejects(
    () => client.listProverReports({ limit: "nope" }),
    /limit must be a positive integer/,
  );
  await assert.rejects(
    () => client.countProverReports({ unknownFilter: true }),
    /unknown prover filter 'unknownFilter'/,
  );
});

test("listProverReports forwards AbortSignal options", async () => {
  const controller = new AbortController();
  let capturedSignal = null;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (_url, init) => {
      capturedSignal = init?.signal ?? null;
      return createResponse({
        status: 200,
        jsonData: [],
        headers: { "content-type": "application/json" },
      });
    },
  });
  await client.listProverReports({}, { signal: controller.signal });
  assert.strictEqual(capturedSignal, controller.signal);
});

test("countProverReports rejects invalid AbortSignal option", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { count: 0 },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.countProverReports({}, { signal: "nope" }),
    /countProverReports options\.signal must be an AbortSignal/,
  );
});

test("getProverReport fetches report by id", async () => {
  const controller = new AbortController();
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/zk/prover/reports/r-1`);
    assert.strictEqual(init?.signal, controller.signal);
    return createResponse({
      status: 200,
      jsonData: {
        id: "r-1",
        ok: true,
        error: null,
        content_type: "text/plain",
        size: 5,
        created_ms: 10,
        processed_ms: 12,
        latency_ms: 2,
        zk1_tags: null,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getProverReport("r-1", { signal: controller.signal });
  assert.deepEqual(result, {
    id: "r-1",
    ok: true,
    error: null,
    content_type: "text/plain",
    size: 5,
    created_ms: 10,
    processed_ms: 12,
    latency_ms: 2,
    zk1_tags: null,
  });
  await assert.rejects(() => client.getProverReport(""), /reportId/);
});

test("deleteProverReport issues delete", async () => {
  let called = false;
  const controller = new AbortController();
  const fetchImpl = async (url, init) => {
    called = true;
    assert.equal(url, `${BASE_URL}/v1/zk/prover/reports/r-2`);
    assert.equal(init.method, "DELETE");
    assert.strictEqual(init.signal, controller.signal);
    return createResponse({ status: 204 });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.deleteProverReport("r-2", { signal: controller.signal });
  assert.ok(called);
  const notFoundClient = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 404 }),
  });
  await notFoundClient.deleteProverReport("missing");
  await assert.rejects(() => client.deleteProverReport(""), /reportId/);
});

test("countProverReports returns parsed count", async () => {
  const controller = new AbortController();
  let capturedSignal = null;
  const fetchImpl = async (url, init) => {
    capturedSignal = init?.signal ?? null;
    assert.ok(url.includes("/v1/zk/prover/reports/count"));
    return createResponse({
      status: 200,
      jsonData: { count: 7 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const count = await client.countProverReports({ failed_only: true }, { signal: controller.signal });
  assert.equal(count, 7);
  assert.strictEqual(capturedSignal, controller.signal);
  const missingPayloadClient = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 200, jsonData: {} }),
  });
  await assert.rejects(
    () => missingPayloadClient.countProverReports(),
    /invalid prover count payload/,
  );
});

test("iterateProverReports paginates with filters and maxItems", async () => {
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    const offset = Number(parsed.searchParams.get("offset") ?? "0");
    const limit = Number(parsed.searchParams.get("limit") ?? "0");
    assert.equal(parsed.searchParams.get("failed_only"), "true");
    assert.equal(limit, 1);
    if (offset >= 2) {
      throw new Error("prover iterator requested too many pages");
    }
    return createResponse({
      status: 200,
      jsonData: [
        {
          id: `rep-${offset}`,
          ok: false,
          error: null,
          content_type: "application/json",
          size: 0,
          created_ms: offset,
          processed_ms: offset + 1,
          latency_ms: 1,
          zk1_tags: null,
        },
      ],
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const ids = [];
  for await (const report of client.iterateProverReports(
    { failedOnly: true },
    { pageSize: 1, maxItems: 2 },
  )) {
    ids.push(typeof report === "string" ? report : report.id);
  }
  assert.deepEqual(ids, ["rep-0", "rep-1"]);
});

test("iterateProverReports rejects unsupported iterator options", () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("unexpected fetch");
    },
  });
  assert.throws(
    () => client.iterateProverReports({ failedOnly: true }, { unexpected: true }),
    /iterator options contains unsupported fields: unexpected/,
  );
});

test("getConnectStatus returns null when disabled", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 404 }),
  });
  const status = await client.getConnectStatus();
  assert.equal(status, null);
});

test("getConnectStatus normalizes payload", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          enabled: true,
          sessions_total: 4,
          sessions_active: 2,
          per_ip_sessions: [{ ip: "127.0.0.1", sessions: 2 }],
          buffered_sessions: 1,
          total_buffer_bytes: 256,
          dedupe_size: 3,
          policy: {
            ws_max_sessions: 64,
            ws_per_ip_max_sessions: 4,
            ws_rate_per_ip_per_min: 60,
            session_ttl_ms: 1000,
            frame_max_bytes: 1024,
            session_buffer_max_bytes: 2048,
            relay_enabled: true,
            relay_strategy: "broadcast",
            relay_effective_strategy: "local_only",
            relay_p2p_attached: false,
            heartbeat_interval_ms: 5000,
            heartbeat_miss_tolerance: 2,
            heartbeat_min_interval_ms: 1000,
          },
          frames_in_total: 10,
          frames_out_total: 20,
          ciphertext_total: 30,
          dedupe_drops_total: 0,
          buffer_drops_total: 1,
          plaintext_control_drops_total: 2,
          monotonic_drops_total: 3,
          sequence_violation_closes_total: 4,
          role_direction_mismatch_total: 5,
          ping_miss_total: 4,
          p2p_rebroadcasts_total: 6,
          p2p_rebroadcast_skipped_total: 7,
        },
        headers: { "content-type": "application/json" },
      }),
  });
  const status = await client.getConnectStatus();
  assert.ok(status);
  assert.equal(status?.sessionsTotal, 4);
  assert.equal(status?.perIpSessions[0]?.ip, "127.0.0.1");
  assert.equal(status?.policy?.wsMaxSessions, 64);
  assert.equal(status?.policy?.relayEnabled, true);
  assert.equal(status?.policy?.relayStrategy, "broadcast");
  assert.equal(status?.policy?.relayEffectiveStrategy, "local_only");
  assert.equal(status?.policy?.relayP2pAttached, false);
  assert.equal(status?.sequenceViolationClosesTotal, 4);
  assert.equal(status?.roleDirectionMismatchTotal, 5);
  assert.equal(status?.p2pRebroadcastsTotal, 6);
  assert.equal(status?.p2pRebroadcastSkippedTotal, 7);
});

test("getConnectStatus preserves relay-disabled effective local-only fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          enabled: true,
          sessions_total: 1,
          sessions_active: 1,
          per_ip_sessions: [{ ip: "127.0.0.1", sessions: 1 }],
          buffered_sessions: 0,
          total_buffer_bytes: 0,
          dedupe_size: 0,
          policy: {
            ws_max_sessions: 64,
            ws_per_ip_max_sessions: 4,
            ws_rate_per_ip_per_min: 60,
            session_ttl_ms: 1000,
            frame_max_bytes: 1024,
            session_buffer_max_bytes: 2048,
            relay_enabled: false,
            relay_strategy: "broadcast",
            relay_effective_strategy: "local_only",
            relay_p2p_attached: true,
            heartbeat_interval_ms: 5000,
            heartbeat_miss_tolerance: 2,
            heartbeat_min_interval_ms: 1000,
          },
          frames_in_total: 1,
          frames_out_total: 1,
          ciphertext_total: 1,
          dedupe_drops_total: 0,
          buffer_drops_total: 0,
          plaintext_control_drops_total: 0,
          monotonic_drops_total: 0,
          sequence_violation_closes_total: 0,
          role_direction_mismatch_total: 0,
          ping_miss_total: 0,
          p2p_rebroadcasts_total: 0,
          p2p_rebroadcast_skipped_total: 0,
        },
        headers: { "content-type": "application/json" },
      }),
  });
  const status = await client.getConnectStatus();
  assert.ok(status);
  assert.equal(status?.policy?.relayEnabled, false);
  assert.equal(status?.policy?.relayStrategy, "broadcast");
  assert.equal(status?.policy?.relayEffectiveStrategy, "local_only");
  assert.equal(status?.policy?.relayP2pAttached, true);
  assert.equal(status?.p2pRebroadcastsTotal, 0);
  assert.equal(status?.p2pRebroadcastSkippedTotal, 0);
});

test("getConnectStatus rejects non-integer policy values", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          enabled: true,
          sessions_total: 1,
          sessions_active: 1,
          per_ip_sessions: [],
          buffered_sessions: 0,
          total_buffer_bytes: 0,
          dedupe_size: 0,
          policy: {
            ws_max_sessions: 64,
            ws_per_ip_max_sessions: 4,
            ws_rate_per_ip_per_min: 60,
            session_ttl_ms: 1000.5,
            frame_max_bytes: 1024,
            session_buffer_max_bytes: 2048,
            relay_enabled: true,
            relay_strategy: "broadcast",
            relay_effective_strategy: "local_only",
            relay_p2p_attached: false,
            heartbeat_interval_ms: 5000,
            heartbeat_miss_tolerance: 2,
            heartbeat_min_interval_ms: 1000,
          },
          frames_in_total: 0,
          frames_out_total: 0,
          ciphertext_total: 0,
          dedupe_drops_total: 0,
          buffer_drops_total: 0,
          plaintext_control_drops_total: 0,
          monotonic_drops_total: 0,
          sequence_violation_closes_total: 0,
          role_direction_mismatch_total: 0,
          ping_miss_total: 0,
          p2p_rebroadcasts_total: 0,
          p2p_rebroadcast_skipped_total: 0,
        },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.getConnectStatus(),
    (error) => {
      assert(error instanceof TypeError);
      assert.equal(error.name, "ValidationError");
      assert.match(error.message, /session_ttl_ms/);
      return true;
    },
  );
});

test("createConnectSession validates sid and posts JSON", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: {
        sid: SAMPLE_CONNECT_SID_BASE64,
        wallet_uri: "iroha://connect",
        app_uri: "iroha://connect/app",
        token_app: "token-app",
        token_wallet: "token-wallet",
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const resp = await client.createConnectSession({
    sid: SAMPLE_CONNECT_SID_BASE64,
    node: "torii",
  });
  assert.equal(resp.sid, SAMPLE_CONNECT_SID_BASE64);
  assert.equal(resp.wallet_uri, "iroha://connect");
  assert.equal(resp.app_uri, "iroha://connect/app");
  assert.equal(resp.token_app, "token-app");
  assert.equal(resp.token_wallet, "token-wallet");
  assert.deepEqual(resp.extra, {});
  assert.equal(captured.url, `${BASE_URL}/v1/connect/session`);
  assert.equal(captured.init.headers["Content-Type"], "application/json");
  assert.deepEqual(JSON.parse(captured.init.body), {
    sid: SAMPLE_CONNECT_SID_BASE64,
    node: "torii",
  });
});

test("createConnectSession rejects malformed responses", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          sid: SAMPLE_CONNECT_SID_BASE64,
          wallet_uri: "iroha://connect",
          app_uri: "iroha://connect/app",
          token_app: "token-app",
        },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.createConnectSession({ sid: SAMPLE_CONNECT_SID_BASE64 }),
    /token_wallet/i,
  );
});

test("createConnectSession accepts base64url sid", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: {
        sid: SAMPLE_CONNECT_SID_BASE64,
        wallet_uri: "iroha://connect",
        app_uri: "iroha://connect/app",
        token_app: "token-app",
        token_wallet: "token-wallet",
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const resp = await client.createConnectSession({ sid: SAMPLE_CONNECT_SID_BASE64 });
  assert.equal(resp.sid, SAMPLE_CONNECT_SID_BASE64);
  assert.equal(captured.url, `${BASE_URL}/v1/connect/session`);
  assert.deepEqual(JSON.parse(captured.init.body), { sid: SAMPLE_CONNECT_SID_BASE64 });
});

test("createConnectSession rejects invalid sid values", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not be called");
    },
  });
  await assert.rejects(
    () => client.createConnectSession({ sid: "not a valid sid" }),
    /32-byte.*(base64url|hex)/i,
  );
});

test("createConnectSession accepts hex sid", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: {
        sid: "ab".repeat(32),
        wallet_uri: "iroha://connect",
        app_uri: "iroha://connect/app",
        token_app: "token-app",
        token_wallet: "token-wallet",
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const resp = await client.createConnectSession({ sid: `0x${"ab".repeat(32)}` });
  assert.equal(resp.sid, "ab".repeat(32));
  assert.equal(captured.url, `${BASE_URL}/v1/connect/session`);
  assert.deepEqual(JSON.parse(captured.init.body), { sid: "ab".repeat(32) });
});

test("deleteConnectSession returns flag based on status", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({ status: 204 });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const ok = await client.deleteConnectSession(SAMPLE_CONNECT_SID_BASE64);
  assert.equal(ok, true);
  assert.equal(
    captured.url,
    `${BASE_URL}/v1/connect/session/${encodeURIComponent(SAMPLE_CONNECT_SID_BASE64)}`,
  );
  assert.equal(captured.init.method, "DELETE");
});

test("deleteConnectSession returns false for missing session", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 404 }),
  });
  const ok = await client.deleteConnectSession(SAMPLE_CONNECT_SID_BASE64);
  assert.equal(ok, false);
});

test("listConnectApps normalizes registry payload", async () => {
  let capturedUrl;
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: {
        items: [
          {
            app_id: "calc.wallet",
            display_name: "Calc Wallet",
            namespaces: ["apps"],
            metadata: { website: "https://calc.example" },
            policy: { allow_guardian: true },
          },
        ],
        total: 1,
        next_cursor: "abc",
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const page = await client.listConnectApps({ limit: "5", cursor: "start" });
  assert.ok(capturedUrl.includes("limit=5"));
  assert.ok(capturedUrl.includes("cursor=start"));
  assert.equal(page.items.length, 1);
  assert.equal(page.items[0].appId, "calc.wallet");
  assert.deepEqual(page.items[0].namespaces, ["apps"]);
  assert.equal(page.nextCursor, "abc");
});

test("listConnectApps rejects invalid AbortSignal option", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not invoke fetch");
    },
  });
  await assert.rejects(
    () =>
      client.listConnectApps({
        // @ts-expect-error runtime validation should reject incorrect signal
        signal: {},
      }),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(error.message, /listConnectApps options\.signal must be an AbortSignal/);
      return true;
    },
  );
});

test("iterateConnectApps paginates using cursors", async () => {
  let callCount = 0;
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/connect/app/apps");
    const limit = Number(parsed.searchParams.get("limit"));
    const cursor = parsed.searchParams.get("cursor");
    callCount += 1;
    if (callCount === 1) {
      assert.equal(limit, 2);
      assert.equal(cursor, null);
      return createResponse({
        status: 200,
        jsonData: {
          items: [
            { app_id: "calc.wallet", namespaces: ["apps"], metadata: {}, policy: {} },
            { app_id: "mint.wallet", namespaces: ["apps"], metadata: {}, policy: {} },
          ],
          next_cursor: "cursor-1",
        },
        headers: { "content-type": "application/json" },
      });
    }
    assert.equal(limit, 2);
    assert.equal(cursor, "cursor-1");
    return createResponse({
      status: 200,
      jsonData: {
        items: [{ app_id: "vault.wallet", namespaces: ["apps"], metadata: {}, policy: {} }],
        next_cursor: null,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const ids = [];
  for await (const app of client.iterateConnectApps({ pageSize: 2 })) {
    ids.push(app.appId);
  }
  assert.deepEqual(ids, ["calc.wallet", "mint.wallet", "vault.wallet"]);
  assert.equal(callCount, 2);
});

test("iterateConnectApps stops once maxItems is reached", async () => {
  let callCount = 0;
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/connect/app/apps");
    const cursor = parsed.searchParams.get("cursor");
    callCount += 1;
    if (cursor) {
      assert.fail("iterator should not request another page after hitting maxItems");
    }
    return createResponse({
      status: 200,
      jsonData: {
        items: [
          { app_id: "calc.wallet", namespaces: ["apps"], metadata: {}, policy: {} },
          { app_id: "mint.wallet", namespaces: ["apps"], metadata: {}, policy: {} },
        ],
        next_cursor: "cursor-1",
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const ids = [];
  for await (const app of client.iterateConnectApps({ pageSize: 5n, maxItems: "1" })) {
    ids.push(app.appId);
  }
  assert.deepEqual(ids, ["calc.wallet"]);
  assert.equal(callCount, 1);
});

test("getConnectApp normalizes record payloads", async () => {
  let capturedUrl;
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: {
        app_id: "calc.wallet",
        display_name: "Calc Wallet",
        metadata: { homepage: "https://calc.example" },
        policy: { relay_enabled: true },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const record = await client.getConnectApp("calc.wallet");
  assert.equal(
    capturedUrl,
    `${BASE_URL}/v1/connect/app/apps/calc.wallet`,
    "connect app path mismatch",
  );
  assert.equal(record.appId, "calc.wallet");
  assert.equal(record.displayName, "Calc Wallet");
  assert.equal(record.metadata.homepage, "https://calc.example");
  assert.equal(record.policy.relay_enabled, true);
});

test("registerConnectApp posts normalized payload", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 201,
      jsonData: {
        app_id: "calc.wallet",
        namespaces: ["apps"],
        metadata: {},
        policy: {},
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const record = await client.registerConnectApp({
    appId: "calc.wallet",
    displayName: "Calc Wallet",
    namespaces: ["apps"],
    metadata: { website: "https://calc.example" },
    policy: { allow_guardian: true },
  });
  assert.equal(captured.url, `${BASE_URL}/v1/connect/app/apps`);
  const body = JSON.parse(captured.init.body);
  assert.equal(body.app_id, "calc.wallet");
  assert.equal(body.display_name, "Calc Wallet");
  assert.deepEqual(body.namespaces, ["apps"]);
  assert.deepEqual(record.appId, "calc.wallet");
});

test("registerConnectApp rejects invalid server payloads", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 201,
      jsonData: { app: "calc.wallet" },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () =>
      client.registerConnectApp({
        appId: "calc.wallet",
        namespaces: ["apps"],
        metadata: {},
        policy: {},
      }),
    /connect app response/,
  );
});

test("deleteConnectApp returns true when record exists", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({ status: 204 });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const removed = await client.deleteConnectApp("calc.wallet");
  assert.equal(removed, true);
  assert.equal(
    captured.url,
    `${BASE_URL}/v1/connect/app/apps/calc.wallet`,
    "delete app path mismatch",
  );
  assert.equal(captured.init.method, "DELETE");
});

test("deleteConnectApp returns false for missing record", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 404 }),
  });
  const removed = await client.deleteConnectApp("missing.wallet");
  assert.equal(removed, false);
});

test("getConnectAppPolicy normalizes controls", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        policy: {
          relay_enabled: true,
          ws_max_sessions: 25,
        },
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const policy = await client.getConnectAppPolicy();
  assert.equal(policy.relayEnabled, true);
  assert.equal(policy.wsMaxSessions, 25);
});

test("getConnectAppPolicy rejects non-integer policy controls", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        policy: {
          relay_enabled: true,
          ws_max_sessions: 25.5,
        },
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getConnectAppPolicy(),
    (error) => {
      assert(error instanceof RangeError);
      assert.match(error.message, /ws_max_sessions/);
      return true;
    },
  );
});

test("updateConnectAppPolicy serializes camelCase updates", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: {
        policy: {
          relay_enabled: true,
          ws_max_sessions: 15,
          session_ttl_ms: 60000,
        },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.updateConnectAppPolicy({
    relayEnabled: true,
    wsMaxSessions: 15,
    sessionTtlMs: 60_000,
  });
  assert.equal(captured.url, `${BASE_URL}/v1/connect/app/policy`);
  const body = JSON.parse(captured.init.body);
  assert.deepEqual(body, {
    relay_enabled: true,
    ws_max_sessions: 15,
    session_ttl_ms: 60_000,
  });
  assert.equal(result.relayEnabled, true);
  assert.equal(result.wsMaxSessions, 15);
  assert.equal(result.sessionTtlMs, 60_000);
});

test("setConnectAdmissionManifest serializes entries", async () => {
  let capturedBody;
  const fetchImpl = async (url, init) => {
    capturedBody = JSON.parse(init.body);
    return createResponse({
      status: 200,
      jsonData: {
        entries: [
          { app_id: "calc.wallet", namespaces: ["apps"], metadata: {}, policy: {} },
        ],
        version: 1,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const manifest = await client.setConnectAdmissionManifest({
    entries: [
      {
        appId: "calc.wallet",
        namespaces: ["apps"],
        metadata: {},
        policy: {},
      },
    ],
    version: 1,
  });
  assert.equal(capturedBody.entries[0].app_id, "calc.wallet");
  assert.equal(manifest.entries[0].appId, "calc.wallet");
});

test("getConnectAdmissionManifest normalizes manifest payloads", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        manifest: {
          version: 2,
          entries: [{ app_id: "calc.wallet", namespaces: ["apps"], policy: {} }],
          manifest_hash: "abcd",
        },
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const manifest = await client.getConnectAdmissionManifest();
  assert.equal(manifest.version, 2);
  assert.equal(manifest.entries.length, 1);
  assert.equal(manifest.entries[0].appId, "calc.wallet");
  assert.equal(manifest.manifestHash, "abcd");
});

test("Connect admin wrappers reject non-object options", async () => {
  const noopFetch = async () => {
    throw new Error("fetch should not be invoked");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl: noopFetch });
  await assert.rejects(
    () => client.getConnectApp("calc.wallet", "bad"),
    /getConnectApp options must be an object/,
  );
  await assert.rejects(
    () =>
      client.registerConnectApp(
        { appId: "calc.wallet", namespaces: ["apps"], metadata: {} },
        123,
      ),
    /registerConnectApp options must be an object/,
  );
  await assert.rejects(
    () => client.getConnectAppPolicy(null),
    /getConnectAppPolicy options must be an object/,
  );
  await assert.rejects(
    () =>
      client.updateConnectAppPolicy(
        { relayEnabled: true, wsMaxSessions: 1, sessionTtlMs: 1000 },
        Symbol("options"),
      ),
    /updateConnectAppPolicy options must be an object/,
  );
  const manifestInput = {
    entries: [{ appId: "calc.wallet", namespaces: ["apps"], metadata: {} }],
    version: 1,
  };
  await assert.rejects(
    () => client.getConnectAdmissionManifest("oops"),
    /getConnectAdmissionManifest options must be an object/,
  );
  await assert.rejects(
    () => client.setConnectAdmissionManifest(manifestInput, false),
    /setConnectAdmissionManifest options must be an object/,
  );
});

test("Connect admin wrappers reject unsupported option fields", async () => {
  const noopFetch = async () => {
    throw new Error("fetch should not be invoked");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl: noopFetch });
  await assert.rejects(
    () => client.listConnectApps({ limit: 1, extra: true }),
    /listConnectApps options contains unsupported fields: extra/,
  );
  await assert.rejects(
    () => client.getConnectApp("calc.wallet", { retry: true }),
    /getConnectApp options contains unsupported fields: retry/,
  );
  await assert.rejects(
    () => client.getConnectAppPolicy({ cache: "nope" }),
    /getConnectAppPolicy options contains unsupported fields: cache/,
  );
});

test("registerContractCode posts manifest JSON", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({ status: 202 });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.registerContractCode({
    authority: FIXTURE_ALICE_ID,
    privateKey: "ed25519:deadbeef",
    manifest: { code_hash: "a".repeat(64), compiler_fingerprint: "rustc" },
    codeBytes: Buffer.from("hello"),
  });
  assert.equal(captured.url, `${BASE_URL}/v1/contracts/code`);
  assert.equal(captured.init.method, "POST");
  assert.equal(captured.init.headers["Content-Type"], "application/json");
  const body = JSON.parse(captured.init.body);
  assert.deepEqual(body, {
    authority: FIXTURE_ALICE_ID,
    private_key: "ed25519:deadbeef",
    manifest: {
      code_hash: "a".repeat(64),
      compiler_fingerprint: "rustc",
      abi_hash: null,
      features_bitmap: null,
      access_set_hints: null,
      entrypoints: null,
    },
    code_bytes: Buffer.from("hello").toString("base64"),
  });
});

test("deployContract submits base64 payload and returns response", async () => {
  let captured;
  const responsePayload = {
    ok: true,
    code_hash_hex: "b".repeat(64),
    abi_hash_hex: "c".repeat(64),
  };
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: responsePayload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.deployContract({
    authority: FIXTURE_ALICE_ID,
    privateKey: "ed25519:deadbeef",
    codeB64: Buffer.from("payload"),
    manifest: { features_bitmap: 1 },
  });
  assert.equal(captured.url, `${BASE_URL}/v1/contracts/deploy`);
  const body = JSON.parse(captured.init.body);
  assert.equal(body.code_b64, Buffer.from("payload").toString("base64"));
  assert.deepEqual(body.manifest, {
    code_hash: null,
    abi_hash: null,
    compiler_fingerprint: null,
    features_bitmap: 1,
    access_set_hints: null,
    entrypoints: null,
  });
  assert.deepEqual(result, responsePayload);
});

test("deployContract rejects invalid base64 payloads", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not fetch");
    },
  });
  await assert.rejects(
    () =>
      client.deployContract({
        authority: FIXTURE_ALICE_ID,
        privateKey: "ed25519:deadbeef",
        codeB64: "YmFzZTY0*",
      }),
    (error) =>
      error instanceof ValidationError &&
      error.code === ValidationErrorCode.INVALID_STRING &&
      /deployContract\.codeB64/.test(error.message),
  );
});

test("deployContract rejects empty code bytes", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not fetch");
    },
  });
  await assert.rejects(
    () =>
      client.deployContract({
        authority: FIXTURE_ALICE_ID,
        privateKey: "ed25519:deadbeef",
        codeB64: Buffer.alloc(0),
      }),
    /deployContract\.codeB64/,
  );
});

test("deployContractInstance posts combined payload", async () => {
  let captured;
  const responsePayload = {
    ok: true,
    namespace: "apps",
    contract_id: "calc",
    code_hash_hex: "d".repeat(64),
    abi_hash_hex: "e".repeat(64),
  };
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: responsePayload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.deployContractInstance({
    authority: FIXTURE_ALICE_ID,
    privateKey: "ed25519:deadbeef",
    namespace: "apps",
    contractId: "calc",
    codeB64: "YmFzZTY0",
    manifest: { access_set_hints: { read_keys: [`account:${FIXTURE_ALICE_ID}`] } },
  });
  assert.equal(captured.url, `${BASE_URL}/v1/contracts/instance`);
  const body = JSON.parse(captured.init.body);
  assert.deepEqual(body, {
    authority: FIXTURE_ALICE_ID,
    private_key: "ed25519:deadbeef",
    namespace: "apps",
    contract_id: "calc",
    code_b64: "YmFzZTY0",
    manifest: {
      code_hash: null,
      abi_hash: null,
      compiler_fingerprint: null,
      features_bitmap: null,
      access_set_hints: { read_keys: [`account:${FIXTURE_ALICE_ID}`], write_keys: [] },
      entrypoints: null,
    },
  });
  assert.deepEqual(result, responsePayload);
});

test("activateContractInstance normalizes code hash", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: { ok: true },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.activateContractInstance({
    authority: FIXTURE_ALICE_ID,
    privateKey: "ed25519:deadbeef",
    namespace: "apps",
    contractId: "calc",
    codeHash: "0x" + "f".repeat(64),
  });
  const body = JSON.parse(captured.init.body);
  assert.equal(body.code_hash, "f".repeat(64));
  assert.deepEqual(result, { ok: true });
});

test("callContract posts payload metadata and normalizes response", async () => {
  let captured;
  const responsePayload = {
    ok: true,
    namespace: "apps",
    contract_id: "calc",
    code_hash_hex: "1".repeat(64),
    abi_hash_hex: "2".repeat(64),
    tx_hash_hex: "3".repeat(64),
    entrypoint: "increment",
  };
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: responsePayload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = { value: 7, labels: ["a", "b"] };
  const result = await client.callContract({
    authority: FIXTURE_ALICE_ID,
    privateKey: "ed25519:deadbeef",
    namespace: "apps",
    contractId: "calc",
    entrypoint: "increment",
    payload,
    gasAssetId: FIXTURE_ASSET_ID_D,
    gasLimit: 42n,
  });
  assert.equal(captured.url, `${BASE_URL}/v1/contracts/call`);
  const body = JSON.parse(captured.init.body);
  assert.deepEqual(body, {
    authority: FIXTURE_ALICE_ID,
    private_key: "ed25519:deadbeef",
    namespace: "apps",
    contract_id: "calc",
    entrypoint: "increment",
    payload,
    gas_asset_id: FIXTURE_ASSET_ID_D,
    gas_limit: 42,
  });
  assert.deepEqual(result, {
    ok: true,
    namespace: "apps",
    contract_id: "calc",
    code_hash_hex: "1".repeat(64),
    abi_hash_hex: "2".repeat(64),
    tx_hash_hex: "3".repeat(64),
    entrypoint: "increment",
  });
});

test("callContract rejects missing gasLimit", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not be invoked");
    },
  });
  await assert.rejects(
    () =>
      client.callContract({
        authority: FIXTURE_ALICE_ID,
        privateKey: "ed25519:deadbeef",
        namespace: "apps",
        contractId: "calc",
      }),
    /contractCall\.gasLimit/,
  );
});

test("callContract rejects non-object options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not be invoked");
    },
  });
  await assert.rejects(
    () =>
      client.callContract(
        {
          authority: FIXTURE_ALICE_ID,
          privateKey: "ed25519:deadbeef",
          namespace: "apps",
          contractId: "calc",
          entrypoint: "ping",
        },
        "invalid",
      ),
    /callContract options must be an object/,
  );
});

test("callContract rejects unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not be invoked");
    },
  });
  await assert.rejects(
    () =>
      client.callContract(
        {
          authority: FIXTURE_ALICE_ID,
          privateKey: "ed25519:deadbeef",
          namespace: "apps",
          contractId: "calc",
          entrypoint: "ping",
        },
        { signal: new AbortController().signal, retry: true },
      ),
    /callContract options contains unsupported fields: retry/,
  );
});

test("proposeMultisigContractCall posts alias selector and normalizes response", async () => {
  let captured;
  const responsePayload = {
    ok: true,
    resolved_multisig_account_id: FIXTURE_ALICE_ID,
    submitted: false,
    proposal_id: "a".repeat(64),
    instructions_hash: "a".repeat(64),
    creation_time_ms: 123456,
    signing_message_b64: "AQ==",
  };
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: responsePayload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.proposeMultisigContractCall({
    multisigAccountAlias: "cbdc@hbl",
    signerAccountId: FIXTURE_ALICE_ID,
    namespace: "apps",
    contractId: "mint",
    entrypoint: "execute",
    payload: { amount: "10" },
    gasAssetId: FIXTURE_ASSET_ID_D,
    feeSponsor: FIXTURE_BOB_ID,
    gasLimit: 5,
  });
  assert.equal(captured.url, `${BASE_URL}/v1/contracts/call/multisig/propose`);
  const body = JSON.parse(captured.init.body);
  assert.deepEqual(body, {
    multisig_account_alias: "cbdc@hbl",
    signer_account_id: FIXTURE_ALICE_ID,
    namespace: "apps",
    contract_id: "mint",
    entrypoint: "execute",
    payload: { amount: "10" },
    gas_asset_id: FIXTURE_ASSET_ID_D,
      fee_sponsor: FIXTURE_BOB_ID,
      gas_limit: 5,
  });
  assert.deepEqual(result, {
    ...responsePayload,
    executed_tx_hash_hex: null,
  });
});

test("approveMultisigContractCall posts concrete selector and normalizes response", async () => {
  let captured;
  const responsePayload = {
    ok: true,
    resolved_multisig_account_id: FIXTURE_ALICE_ID,
    submitted: true,
    proposal_id: "b".repeat(64),
    instructions_hash: "b".repeat(64),
    executed_tx_hash_hex: "c".repeat(64),
  };
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: responsePayload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.approveMultisigContractCall({
    multisigAccountId: FIXTURE_ALICE_ID,
    signerAccountId: FIXTURE_BOB_ID,
    proposalId: "b".repeat(64),
    signatureB64: "AQ==",
  });
  assert.equal(captured.url, `${BASE_URL}/v1/contracts/call/multisig/approve`);
  const body = JSON.parse(captured.init.body);
  assert.deepEqual(body, {
    multisig_account_id: FIXTURE_ALICE_ID,
    signer_account_id: FIXTURE_BOB_ID,
      proposal_id: "b".repeat(64),
      signature_b64: "AQ==",
  });
  assert.deepEqual(result, {
    ...responsePayload,
    creation_time_ms: null,
    signing_message_b64: null,
  });
});

test("getMultisigSpec posts selector and returns raw spec payload", async () => {
  let captured;
  const responsePayload = {
    resolved_multisig_account_id: FIXTURE_ALICE_ID,
    spec: {
      signatories: [FIXTURE_ALICE_ID, FIXTURE_BOB_ID],
      quorum: 2,
      transaction_ttl_ms: 60000,
    },
  };
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: responsePayload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getMultisigSpec({
    multisig_account_alias: "cbdc@ubl",
  });
  assert.equal(captured.url, `${BASE_URL}/v1/multisig/spec`);
  assert.deepEqual(JSON.parse(captured.init.body), {
    multisig_account_alias: "cbdc@ubl",
  });
  assert.deepEqual(result, responsePayload);
});

test("listMultisigProposals decodes proposal entries", async () => {
  const responsePayload = {
    resolved_multisig_account_id: FIXTURE_ALICE_ID,
    proposals: [
      {
        proposal_id: "d".repeat(64),
        instructions_hash: "d".repeat(64),
        proposal: {
          approvals: [FIXTURE_ALICE_ID],
          proposed_at_ms: 42,
        },
      },
    ],
  };
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: responsePayload,
        headers: { "content-type": "application/json" },
      }),
  });
  const result = await client.listMultisigProposals({
    multisigAccountAlias: "cbdc@hbl",
  });
  assert.deepEqual(result, responsePayload);
});

test("getMultisigProposal resolves by instructions hash", async () => {
  let captured;
  const responsePayload = {
    resolved_multisig_account_id: FIXTURE_ALICE_ID,
    proposal_id: "e".repeat(64),
    instructions_hash: "e".repeat(64),
    proposal: {
      approvals: [FIXTURE_ALICE_ID, FIXTURE_BOB_ID],
      proposed_at_ms: 43,
    },
  };
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: responsePayload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getMultisigProposal({
    multisigAccountAlias: "cbdc@hbl",
    instructionsHash: "e".repeat(64),
  });
  assert.equal(captured.url, `${BASE_URL}/v1/multisig/proposals/get`);
  assert.deepEqual(JSON.parse(captured.init.body), {
    multisig_account_alias: "cbdc@hbl",
    instructions_hash: "e".repeat(64),
  });
  assert.deepEqual(result, responsePayload);
});

test("getMultisigSpec rejects selectors that set both account id and alias", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not be invoked");
    },
  });
  await assert.rejects(
    () =>
      client.getMultisigSpec({
        multisigAccountId: FIXTURE_ALICE_ID,
        multisigAccountAlias: "cbdc@hbl",
      }),
    /requires exactly one of multisig_account_id or multisig_account_alias/,
  );
});

test("getContractManifest returns normalized payload", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        manifest: { code_hash: "0".repeat(64), abi_hash: null },
        code_bytes: null,
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const manifest = await client.getContractManifest("0".repeat(64));
  assert.ok(manifest);
  assert.equal(manifest?.manifest.code_hash, "0".repeat(64));
  assert.equal(manifest?.manifest.abi_hash ?? null, null);
  assert.equal(manifest?.code_bytes, null);
});

test("getContractManifest returns null on 404", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 404 }),
  });
  const result = await client.getContractManifest("0".repeat(64));
  assert.equal(result, null);
});

test("getContractCodeBytes returns record", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: { code_b64: "Y29kZQ==" },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getContractCodeBytes("1".repeat(64));
  assert.deepEqual(result, { code_b64: "Y29kZQ==" });
});

test("listContractInstances encodes query params", async () => {
  const fetchImpl = async (url) => {
    assert.ok(url.includes("/v1/contracts/instances/apps"));
    assert.ok(url.includes("contains=calc"));
    assert.ok(url.includes("limit=5"));
    assert.ok(url.includes("hash_prefix=aa"));
    return createResponse({
      status: 200,
      jsonData: {
        namespace: "apps",
        total: 1,
        offset: 0,
        limit: 5,
        instances: [{ contract_id: "calc", code_hash_hex: "1".repeat(64) }],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.listContractInstances("apps", {
    contains: "calc",
    hashPrefix: "aa",
    limit: 5,
  });
  assert.deepEqual(result, {
    namespace: "apps",
    total: 1,
    offset: 0,
    limit: 5,
    instances: [{ contract_id: "calc", code_hash_hex: "1".repeat(64) }],
  });
});

test("listGovernanceInstances mirrors query and response handling", async () => {
  let calledUrl;
  const fetchImpl = async (url) => {
    calledUrl = url;
    return createResponse({
      status: 200,
      jsonData: {
        namespace: "apps",
        total: "2",
        offset: "1",
        limit: "10",
        instances: [
          { contract_id: "calc.v1", code_hash_hex: "1".repeat(64) },
          { contract_id: "calc.v2", code_hash_hex: "2".repeat(64) },
        ],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.listGovernanceInstances("apps", {
    contains: "calc",
    hashPrefix: "12",
    offset: 1,
    limit: 10,
    order: "HASH_DESC",
  });
  assert.ok(calledUrl?.includes("/v1/gov/instances/apps"));
  assert.ok(calledUrl?.includes("contains=calc"));
  assert.ok(calledUrl?.includes("hash_prefix=12"));
  assert.ok(calledUrl?.includes("offset=1"));
  assert.ok(calledUrl?.includes("limit=10"));
  assert.ok(calledUrl?.includes("order=hash_desc"));
  assert.equal(result.namespace, "apps");
  assert.equal(result.total, 2);
  assert.equal(result.offset, 1);
  assert.equal(result.limit, 10);
  assert.equal(result.instances.length, 2);
  assert.equal(result.instances[0].contract_id, "calc.v1");
});

test("listContractInstances rejects non-object options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not fetch");
    },
  });
  await assert.rejects(
    () => client.listContractInstances("apps", null),
    /contractInstances options must be an object/,
  );
  await assert.rejects(
    () => client.listContractInstances("apps", 7),
    /contractInstances options must be an object/,
  );
});

test("listGovernanceInstances rejects invalid order values", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { namespace: "apps", total: 0, offset: 0, limit: 10, instances: [] },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.listGovernanceInstances("apps", { order: "recent_first" }),
    /contractInstances\.order must be one of/,
  );
});

test("listGovernanceInstances rejects non-hex hashPrefix values", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: { namespace: "apps", total: 0, offset: 0, limit: 10, instances: [] },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () => client.listGovernanceInstances("apps", { hashPrefix: "zzzz" }),
    /contractInstances\.hashPrefix must be a non-empty hexadecimal string/,
  );
});

test("listGovernanceInstances rejects unsupported option keys", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not be invoked for invalid options");
    },
  });
  await assert.rejects(
    () => client.listGovernanceInstances("apps", { contains: "calc", cursor: "abc" }),
    /contractInstances options contains unsupported fields: cursor/,
  );
});

test("listContractInstances rejects invalid signal values", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listContractInstances("apps", { signal: {} }),
    /contractInstances options\.signal must be an AbortSignal/,
  );
});

test("listTriggers encodes query params and normalizes payload", async () => {
  let capturedUrl;
  const authority = normalizeAccountId(
    FIXTURE_AUTHORITY_ID,
    "listTriggers.authority",
  );
  const triggerPayload = {
    id: "apps::mint_rewards",
    action: { Mint: { Asset: { object: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM" } } },
    metadata: { label: "demo" },
  };
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: { items: [triggerPayload], total: "1" },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const page = await client.listTriggers({
    namespace: "apps",
    authority,
    limit: 5,
    offset: 10,
  });
  const parsed = new URL(capturedUrl);
  assert.equal(parsed.pathname, "/v1/triggers");
  assert.equal(parsed.searchParams.get("namespace"), "apps");
  assert.equal(parsed.searchParams.get("authority"), authority);
  assert.equal(parsed.searchParams.get("limit"), "5");
  assert.equal(parsed.searchParams.get("offset"), "10");
  assert.equal(page.total, 1);
  assert.equal(page.items[0].id, triggerPayload.id);
  assert.deepEqual(page.items[0].action, triggerPayload.action);
  assert.deepEqual(page.items[0].metadata, triggerPayload.metadata);
  assert.deepEqual(page.items[0].raw, triggerPayload);
});

test("listTriggers rejects invalid signal values", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listTriggers({ signal: {} }),
    /triggers options\.signal must be an AbortSignal/,
  );
});

test("getTrigger validates options before network access", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.getTrigger("apps::mint_rewards", 123),
    /getTrigger options must be an object/,
  );
  await assert.rejects(
    () => client.getTrigger("apps::mint_rewards", { signal: {} }),
    /getTrigger options\.signal must be an AbortSignal/,
  );
  await assert.rejects(
    () => client.getTrigger("apps::mint_rewards", { extra: "nope" }),
    /getTrigger options contains unsupported fields: extra/,
  );
});

test("getTrigger handles 404 and normalizes metadata", async () => {
  let calls = 0;
  const payload = {
    id: "apps::mint_rewards",
    action: { Mint: { Asset: { object: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM" } } },
  };
  const fetchImpl = async () => {
    calls += 1;
    if (calls === 1) {
      return createResponse({ status: 404 });
    }
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const missing = await client.getTrigger("apps::missing");
  assert.equal(missing, null);
  const record = await client.getTrigger("apps::mint_rewards");
  assert.ok(record);
  assert.equal(record.id, payload.id);
  assert.deepEqual(record.action, payload.action);
  assert.deepEqual(record.metadata, {});
});

test("registerTrigger posts JSON body", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 202,
      jsonData: { ok: true },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const response = await client.registerTrigger({
    id: "apps::rotate_peer",
    namespace: "apps",
    action: { Mint: {} },
  });
  assert.equal(captured.url, `${BASE_URL}/v1/triggers`);
  assert.equal(captured.init.method, "POST");
  assert.equal(captured.init.headers["Content-Type"], "application/json");
  assert.equal(captured.init.headers.Accept, "application/json");
  assert.deepEqual(JSON.parse(captured.init.body), {
    id: "apps::rotate_peer",
    namespace: "apps",
    action: { Mint: {} },
  });
  assert.deepEqual(response, { ok: true });
});

test("registerTrigger validates options before dispatch", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = { id: "apps::rotate_peer", namespace: "apps", action: { Mint: {} } };
  await assert.rejects(
    () => client.registerTrigger(payload, "oops"),
    /registerTrigger options must be an object/,
  );
  await assert.rejects(
    () => client.registerTrigger(payload, { signal: {} }),
    /registerTrigger options\.signal must be an AbortSignal/,
  );
  await assert.rejects(
    () => client.registerTrigger(payload, { memo: "not-allowed" }),
    /registerTrigger options contains unsupported fields: memo/,
  );
});

test("registerTrigger normalizes base64 actions and metadata", async () => {
  let captured;
  const fetchImpl = async (_url, init) => {
    captured = init;
    return createResponse({ status: 202 });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.registerTrigger({
    id: "apps::encoded_trigger",
    namespace: "apps",
    action: "  AAECAwQ=  ",
    metadata: { window: 4n, labels: ["demo"] },
  });
  const payload = JSON.parse(captured.body);
  assert.equal(payload.action, "AAECAwQ=");
  assert.deepEqual(payload.metadata, { window: "4", labels: ["demo"] });
  assert.equal(payload.namespace, "apps");
});

test("registerTrigger rejects invalid base64 actions", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not fetch");
    },
  });
  await assert.rejects(
    () =>
      client.registerTrigger({
        id: "apps::bad_action",
        namespace: "apps",
        action: "AAAA====",
      }),
    /registerTrigger\.action must be a valid base64 string/,
  );
});

test("registerTrigger rejects invalid payloads", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not reach fetch when validation fails");
    },
  });
  await assert.rejects(
    () => client.registerTrigger({ id: "", action: { Mint: {} } }),
    /registerTrigger\.id/,
  );
  await assert.rejects(
    () =>
      client.registerTrigger({
        id: "apps::bad",
        action: { Mint: {} },
        metadata: "oops",
      }),
    /registerTrigger\.metadata/,
  );
  await assert.rejects(
    () =>
      client.registerTrigger({
        id: "apps::missing_action",
      }),
    /registerTrigger\.action is required/,
  );
});

test("registerTriggerTyped normalizes response payloads", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/triggers`);
    assert.equal(init.method, "POST");
    return createResponse({
      status: 202,
      jsonData: {
        ok: true,
        trigger_id: "apps::mint_rewards",
        tx_instructions: [
          {
            wire_id: "RegisterTrigger",
            payload_hex: "0xDEADBEEF",
          },
        ],
        accepted: true,
        message: "queued",
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const draft = await client.registerTriggerTyped({
    id: "apps::mint_rewards",
    namespace: "apps",
    action: { Mint: {} },
  });
  assert(draft);
  assert.equal(draft.trigger_id, "apps::mint_rewards");
  assert.equal(draft.ok, true);
  assert.equal(draft.accepted, true);
  assert.equal(draft.message, "queued");
  assert.deepEqual(draft.tx_instructions, [
    { wire_id: "RegisterTrigger", payload_hex: "deadbeef" },
  ]);
});

test("deleteTriggerTyped returns null when Torii omits payloads", async () => {
  const fetchImpl = async (url, init) => {
    const encoded = encodeURIComponent("apps::archived");
    assert.equal(url, `${BASE_URL}/v1/triggers/${encoded}`);
    assert.equal(init.method, "DELETE");
    return createResponse({ status: 204 });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.deleteTriggerTyped("apps::archived");
  assert.equal(result, null);
});

test("queryTriggers posts iterable envelope", async () => {
  let capturedBody;
  const fetchImpl = async (_url, init) => {
    capturedBody = JSON.parse(init.body);
    return createResponse({
      status: 200,
      jsonData: { items: [], total: 0 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.queryTriggers({
    filter: { Eq: ["namespace", "apps"] },
    sort: [{ key: "created_at", order: "desc" }],
    limit: 5,
    offset: 2,
    fetchSize: 25,
    queryName: "recent-triggers",
  });
  assert.deepEqual(capturedBody.pagination, { offset: 2, limit: 5 });
  assert.deepEqual(capturedBody.filter, { Eq: ["namespace", "apps"] });
  assert.deepEqual(capturedBody.sort, [{ key: "created_at", order: "desc" }]);
  assert.equal(capturedBody.fetch_size, 25);
  assert.equal(capturedBody.query, "recent-triggers");
  assert.deepEqual(result.items, []);
  assert.equal(result.total, 0);
});

test("queryTriggers normalizes alias fields", async () => {
  let capturedBody;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (_url, init) => {
      capturedBody = JSON.parse(init.body);
      return createResponse({
        status: 200,
        jsonData: { items: [], total: 0 },
        headers: { "content-type": "application/json" },
      });
    },
  });
  const result = await client.queryTriggers({
    fetch_size: 3,
    query_name: "alias-query",
  });
  assert.equal(capturedBody.fetch_size, 3);
  assert.equal(capturedBody.query, "alias-query");
  assert.equal(capturedBody.canonical_i105, undefined);
  assert.deepEqual(result.items, []);
  assert.equal(result.total, 0);
});

test("queryTriggers rejects non-object options", async () => {
  let fetchCalled = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      fetchCalled = true;
      return createResponse({
        status: 200,
        jsonData: { items: [], total: 0 },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await assert.rejects(
    () => client.queryTriggers(5),
    /trigger query options must be a plain object/,
  );
  assert.equal(fetchCalled, false);
});

test("queryTriggers rejects unsupported option keys", async () => {
  let fetchCalled = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      fetchCalled = true;
      return createResponse({
        status: 200,
        jsonData: { items: [], total: 0 },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await assert.rejects(
    () => client.queryTriggers({ limit: 1, extra: true }),
    /trigger query options contains unsupported fields: extra/,
  );
  assert.equal(fetchCalled, false);
});

test("listOfflineAllowances normalizes payloads and query params", async () => {
  let capturedUrl;
  const allowanceRecord = {
    certificate: {
      controller: FIXTURE_ALICE_ID,
      allowance: { asset: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1", amount: "500" },
    },
    current_commitment: "0xdeadbeef",
    registered_at_ms: 1234,
    remaining_amount: "499",
    counter_state: {},
    metadata: {
      "android.integrity.policy": "ProViSiOnEd",
      "android.provisioned.inspector_public_key":
        "ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4",
      "android.provisioned.manifest_schema": "offline_provisioning_v1",
      "android.provisioned.manifest_version": "2",
      "android.provisioned.max_manifest_age_ms": "604800000",
      "android.provisioned.manifest_digest":
        "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
    },
  };
  const minimalAllowanceRecord = { remaining_amount: "125" };
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: {
        items: [
      {
        certificate_id_hex: "cafebabe",
        controller_id: FIXTURE_ALICE_ID,
        controller_display: "soraqqqqqqqq",
        asset_id: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        asset_definition_id: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        asset_definition_name: "USD",
        asset_definition_alias: "usd#main",
            registered_at_ms: "1234",
            expires_at_ms: "9999",
            policy_expires_at_ms: "10001",
            refresh_at_ms: "1500",
            verdict_id_hex: "deadbeef",
        attestation_nonce_hex: "beadfeed",
        remaining_amount: "499.25",
        deadline_kind: "policy",
        deadline_state: "warning",
        deadline_ms: 50_000,
        deadline_ms_remaining: -5_000,
        record: allowanceRecord,
      },
          {
            certificate_id_hex: "feedface",
            controller_id: FIXTURE_BOB_ID,
            controller_display: "soraqqqqqqqr",
            asset_id: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
            asset_definition_id: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
            asset_definition_name: "USD",
            asset_definition_alias: null,
            registered_at_ms: 2000,
            expires_at_ms: 3000,
            policy_expires_at_ms: 4000,
            remaining_amount: "125",
            record: minimalAllowanceRecord,
          },
        ],
        total: "2",
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const filter = { Eq: ["controller_id", FIXTURE_ALICE_ID] };
  const page = await client.listOfflineAllowances({
    filter,
    sort: "registered_at_ms:desc",
    limit: 5,
    offset: 10,
  });
  const parsed = new URL(capturedUrl);
  assert.equal(parsed.pathname, "/v1/offline/allowances");
  assert.equal(parsed.searchParams.get("limit"), "5");
  assert.equal(parsed.searchParams.get("offset"), "10");
  assert.equal(parsed.searchParams.get("canonical_i105"), null);
  assert.equal(parsed.searchParams.get("sort"), "registered_at_ms:desc");
  assert.equal(parsed.searchParams.get("filter"), JSON.stringify(filter));
  assert.equal(page.total, 2);
  assert.equal(page.items.length, 2);
  const item = page.items[0];
  assert.equal(item.certificate_id_hex, "cafebabe");
  assert.equal(item.controller_id, FIXTURE_ALICE_ID);
  assert.equal(item.controller_display, "soraqqqqqqqq");
  assert.equal(item.asset_id, "7EAD8EFYUx1aVKZPUU1fyKvr8dF1");
  assert.equal(item.asset_definition_id, "7EAD8EFYUx1aVKZPUU1fyKvr8dF1");
  assert.equal(item.asset_definition_name, "USD");
  assert.equal(item.asset_definition_alias, "usd#main");
  assert.equal(item.registered_at_ms, 1234);
  assert.equal(item.expires_at_ms, 9999);
  assert.equal(item.policy_expires_at_ms, 10001);
  assert.equal(item.refresh_at_ms, 1500);
  assert.equal(item.verdict_id_hex, "deadbeef");
  assert.equal(item.attestation_nonce_hex, "beadfeed");
  assert.equal(item.remaining_amount, "499.25");
  assert.equal(item.deadline_kind, "policy");
  assert.equal(item.deadline_state, "warning");
  assert.equal(item.deadline_ms, 50_000);
  assert.equal(item.deadline_ms_remaining, -5_000);
  assert.deepEqual(item.record, allowanceRecord);
  assert.deepEqual(item.integrity_metadata, {
    policy: "provisioned",
    provisioned: {
      inspector_public_key:
        "ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4",
      manifest_schema: "offline_provisioning_v1",
      manifest_version: 2,
      max_manifest_age_ms: 604800000,
      manifest_digest_hex:
        "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
    },
  });
  const fallback = page.items[1];
  assert.equal(fallback.refresh_at_ms, null);
  assert.equal(fallback.verdict_id_hex, null);
  assert.equal(fallback.attestation_nonce_hex, null);
  assert.equal(fallback.deadline_kind, null);
  assert.equal(fallback.deadline_state, null);
  assert.equal(fallback.deadline_ms, null);
  assert.equal(fallback.deadline_ms_remaining, null);
  assert.equal(fallback.remaining_amount, "125");
  assert.equal(fallback.asset_definition_alias, null);
  assert.equal(fallback.integrity_metadata, null);
});

test("listOfflineAllowances captures play integrity metadata", async () => {
  const playIntegrityRecord = {
    certificate: {
      metadata: {
        "android.integrity.policy": "play_integrity",
        "android.play_integrity.cloud_project_number": 424242,
        "android.play_integrity.environment": "production",
        "android.play_integrity.package_names": ["tech.iroha.wallet"],
        "android.play_integrity.signing_digests_sha256": [
          "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
        ],
        "android.play_integrity.allowed_app_verdicts": ["play_recognized"],
        "android.play_integrity.allowed_device_verdicts": ["strong"],
        "android.play_integrity.max_token_age_ms": 30000,
      },
    },
    remaining_amount: "7",
  };
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        items: [
          {
            certificate_id_hex: "cafebabe",
            controller_id: FIXTURE_ALICE_ID,
            controller_display: "soraqqqqqqqq",
            asset_id: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
            asset_definition_id: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
            asset_definition_name: "USD",
            asset_definition_alias: null,
            registered_at_ms: "1234",
            expires_at_ms: "9999",
            policy_expires_at_ms: "10001",
            remaining_amount: "7",
            record: playIntegrityRecord,
          },
        ],
        total: 1,
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const page = await client.listOfflineAllowances();
  assert.equal(page.items.length, 1);
  assert.deepEqual(page.items[0].integrity_metadata, {
    policy: "play_integrity",
    play_integrity: {
      cloud_project_number: 424242,
      environment: "production",
      package_names: ["tech.iroha.wallet"],
      signing_digests_sha256: [
        "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
      ],
      allowed_app_verdicts: ["play_recognized"],
      allowed_device_verdicts: ["strong"],
      max_token_age_ms: 30000,
    },
  });
});

test("listOfflineAllowances captures hms safety detect metadata", async () => {
  const hmsRecord = {
    certificate: {
      metadata: {
        "android.integrity.policy": "hms_safety_detect",
        "android.hms_safety_detect.app_id": "103000042",
        "android.hms_safety_detect.package_names": ["tech.iroha.wallet"],
        "android.hms_safety_detect.signing_digests_sha256": [
          "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789",
        ],
        "android.hms_safety_detect.required_evaluations": ["strong_integrity"],
        "android.hms_safety_detect.max_token_age_ms": 3600000,
      },
    },
    remaining_amount: "7",
  };
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: {
        items: [
          {
            certificate_id_hex: "deadbeef",
            controller_id: FIXTURE_BOB_ID,
            controller_display: "soraqqqqqqqq",
            asset_id: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
            asset_definition_id: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
            asset_definition_name: "USD",
            asset_definition_alias: null,
            registered_at_ms: "1234",
            expires_at_ms: "9999",
            policy_expires_at_ms: "10001",
            remaining_amount: "7",
            record: hmsRecord,
          },
        ],
        total: 1,
      },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const page = await client.listOfflineAllowances();
  assert.equal(page.items.length, 1);
  assert.deepEqual(page.items[0].integrity_metadata, {
    policy: "hms_safety_detect",
    hms_safety_detect: {
      app_id: "103000042",
      package_names: ["tech.iroha.wallet"],
      signing_digests_sha256: [
        "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789",
      ],
      required_evaluations: ["strong_integrity"],
      max_token_age_ms: 3600000,
    },
  });
});

test("listOfflineTransfers normalizes payloads and metadata", async () => {
  let capturedUrl = null;
  const assetId = "norito:DEADBEEF";
  const normalizedAssetId = "norito:deadbeef";
  const receiverId = normalizeAccountId(
    FIXTURE_VAULT_ID,
    "listOfflineTransfers.receiver_id",
  );
  const depositAccountId = normalizeAccountId(
    FIXTURE_MERCHANT_ID,
    "listOfflineTransfers.deposit_account_id",
  );
  const transferRecord = {
    metadata: {
      "android.integrity.policy": "Provisioned",
      "android.provisioned.inspector_public_key":
        "ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4",
      "android.provisioned.manifest_schema": "offline_provisioning_v1",
      "android.provisioned.manifest_version": 3,
      "android.provisioned.max_manifest_age_ms": 604800000,
      "android.provisioned.manifest_digest":
        "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
    },
  };
  const minimalTransferRecord = {};
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: {
        items: [
          {
            bundle_id_hex: "CAFEBABE",
            controller_id: FIXTURE_ALICE_ID,
            controller_display: "soraqqqqqqqq",
            receiver_id: FIXTURE_VAULT_ID,
            receiver_display: "soraqqqqqqqr",
            deposit_account_id: FIXTURE_VAULT_ID,
            deposit_account_display: "soraqqqqqqqs",
            asset_id: normalizedAssetId,
            receipt_count: "2",
            total_amount: "15",
            claimed_delta: "15",
            status: "pending",
            recorded_at_ms: "1234",
            recorded_at_height: "2468",
            archived_at_height: null,
            certificate_id_hex: "deadbeef",
            certificate_expires_at_ms: "9999",
            policy_expires_at_ms: "10001",
            refresh_at_ms: "1500",
            verdict_id_hex: "feedface",
            attestation_nonce_hex: "beadfeed",
            platform_policy: "play_integrity",
            platform_token_snapshot: {
              policy: "play_integrity",
              attestation_jws_b64: Buffer.from("token").toString("base64"),
            },
            transfer: transferRecord,
          },
          {
            bundle_id_hex: "FEEDBEEF",
            controller_id: FIXTURE_BOB_ID,
            controller_display: "soraqqqqqqqt",
            receiver_id: FIXTURE_VAULT_ID,
            receiver_display: "soraqqqqqqqu",
            deposit_account_id: FIXTURE_VAULT_ID,
            deposit_account_display: "soraqqqqqqqv",
            asset_id: null,
            receipt_count: 1,
            total_amount: "5",
            claimed_delta: "5",
            status: "applied",
            recorded_at_ms: 2000,
            recorded_at_height: 4000,
            archived_at_height: 5000,
            certificate_id_hex: null,
            certificate_expires_at_ms: null,
            policy_expires_at_ms: null,
            refresh_at_ms: null,
            verdict_id_hex: null,
            attestation_nonce_hex: null,
            platform_policy: null,
            platform_token_snapshot: null,
            transfer: minimalTransferRecord,
          },
        ],
        total: "2",
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const page = await client.listOfflineTransfers({
    limit: 5,
    offset: 2,
    sort: "recorded_at_ms:desc",
    controllerId: FIXTURE_ALICE_ID,
    receiverId,
    depositAccountId,
    assetId,
    platformPolicy: "PLAY_INTEGRITY",
  });
  assert.ok(capturedUrl, "request not issued");
  const parsed = new URL(capturedUrl);
  assert.equal(parsed.pathname, "/v1/offline/transfers");
  assert.equal(parsed.searchParams.get("limit"), "5");
  assert.equal(parsed.searchParams.get("offset"), "2");
  assert.equal(parsed.searchParams.get("sort"), "recorded_at_ms:desc");
  assert.equal(parsed.searchParams.get("controller_id"), FIXTURE_ALICE_ID);
  assert.equal(parsed.searchParams.get("receiver_id"), receiverId);
  assert.equal(parsed.searchParams.get("deposit_account_id"), depositAccountId);
  assert.equal(parsed.searchParams.get("asset_id"), normalizedAssetId);
  assert.equal(parsed.searchParams.get("platform_policy"), "play_integrity");
  assert.equal(page.total, 2);
  const [transfer, fallback] = page.items;
  assert.equal(transfer.bundle_id_hex, "CAFEBABE");
  assert.equal(transfer.controller_id, FIXTURE_ALICE_ID);
  assert.equal(transfer.receiver_id, receiverId);
  assert.equal(transfer.asset_id, normalizedAssetId);
  assert.equal(transfer.receipt_count, 2);
  assert.equal(transfer.total_amount, "15");
  assert.equal(transfer.platform_policy, "play_integrity");
  assert.deepEqual(transfer.integrity_metadata, {
    policy: "provisioned",
    provisioned: {
      inspector_public_key:
        "ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4",
      manifest_schema: "offline_provisioning_v1",
      manifest_version: 3,
      max_manifest_age_ms: 604800000,
      manifest_digest_hex:
        "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
    },
  });
  assert.equal(fallback.integrity_metadata, null);
});

test("issueOfflineCertificate posts draft and parses response", async () => {
  const certId = "deadbeef".repeat(8);
  const draft = {
    controller: FIXTURE_ALICE_ID,
    allowance: {
      asset: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
      amount: "10",
      commitment: Buffer.from([1, 2, 3]),
    },
    spend_public_key: "ed0120deadbeef",
    attestation_report: new Uint8Array([4, 5, 6]),
    issued_at_ms: 100,
    expires_at_ms: 200,
    policy: {
      max_balance: "10",
      max_tx_value: "5",
      expires_at_ms: 200,
    },
  };
  let capturedRequest = null;
  const fetchImpl = async (url, init) => {
    capturedRequest = { url, init };
    return createResponse({
      status: 200,
      jsonData: {
        certificate_id_hex: certId,
        certificate: {
          controller: FIXTURE_ALICE_ID,
          operator: FIXTURE_AUTHORITY_ID,
          allowance: {
            asset: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
            amount: "10",
            commitment: [1, 2, 3],
          },
          spend_public_key: "ed0120deadbeef",
          attestation_report: [4, 5, 6],
          issued_at_ms: 100,
          expires_at_ms: 200,
          policy: {
            max_balance: "10",
            max_tx_value: "5",
            expires_at_ms: 200,
          },
          operator_signature: "AA",
          metadata: {},
          verdict_id: null,
          attestation_nonce: null,
          refresh_at_ms: null,
        },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const response = await client.issueOfflineCertificate(draft);
  assert.ok(capturedRequest, "request not captured");
  assert.equal(new URL(capturedRequest.url).pathname, "/v1/offline/certificates/issue");
  const body = JSON.parse(capturedRequest.init.body);
  assert.ok(body.certificate);
  assert.deepEqual(body.certificate.attestation_report, [4, 5, 6]);
  assert.equal(FIXTURE_AUTHORITY_ID in body.certificate, false);
  assert.equal(response.certificate_id_hex, certId);
  assert.equal(response.certificate.controller, FIXTURE_ALICE_ID);
});

test("submitOfflineSettlement posts transfer and parses response", async () => {
  let capturedRequest = null;
  const fetchImpl = async (url, init) => {
    capturedRequest = { url, init };
    return createResponse({
      status: 200,
      jsonData: {
        bundle_id_hex: "deadbeef",
        transaction_hash_hex:
          "cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe",
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const response = await client.submitOfflineSettlement({
    authority: FIXTURE_AUTHORITY_ID,
    privateKey: Buffer.alloc(32, 2),
    transfer: {
      bundle_id: "deadbeef",
      receipts: [],
    },
  });
  assert.ok(capturedRequest, "request not captured");
  assert.equal(new URL(capturedRequest.url).pathname, "/v1/offline/settlements");
  const body = JSON.parse(capturedRequest.init.body);
  assert.equal(body.authority, FIXTURE_AUTHORITY_ID);
  assert.ok(body.private_key.startsWith("ed25519:"));
  assert.equal(body.transfer.bundle_id, "deadbeef");
  assert.equal("build_claim_overrides" in body, false);
  assert.equal("repair_existing_build_claims" in body, false);
  assert.equal(response.bundle_id_hex, "deadbeef");
  assert.equal(
    response.transaction_hash_hex,
    "cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe",
  );
});

test("submitOfflineSettlement accepts build claim overrides and repair flag", async () => {
  const overrideTxIdHex = "ab".repeat(32);
  let capturedRequest = null;
  const fetchImpl = async (url, init) => {
    capturedRequest = { url, init };
    return createResponse({
      status: 200,
      jsonData: { bundle_id_hex: "deadbeef" },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.submitOfflineSettlement({
    authority: FIXTURE_AUTHORITY_ID,
    privateKey: "ed25519:deadbeef",
    transfer: {
      bundle_id: "deadbeef",
      receipts: [],
    },
    buildClaimOverrides: [
      {
        txIdHex: overrideTxIdHex,
        appId: "com.example.android",
        buildNumber: 77,
        issuedAtMs: 1700000000000,
        expiresAtMs: 1700000100000,
      },
    ],
    repairExistingBuildClaims: true,
  });
  assert.ok(capturedRequest, "request not captured");
  const body = JSON.parse(capturedRequest.init.body);
  assert.equal(body.repair_existing_build_claims, true);
  assert.equal(Array.isArray(body.build_claim_overrides), true);
  assert.equal(body.build_claim_overrides.length, 1);
  assert.equal(body.build_claim_overrides[0].tx_id_hex, overrideTxIdHex);
  assert.equal(body.build_claim_overrides[0].app_id, "com.example.android");
  assert.equal(body.build_claim_overrides[0].build_number, 77);
  assert.equal(body.build_claim_overrides[0].issued_at_ms, 1700000000000);
  assert.equal(body.build_claim_overrides[0].expires_at_ms, 1700000100000);
});

test("submitOfflineSettlement validates build claim override shape", async () => {
  let fetchCalled = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      fetchCalled = true;
      return createResponse({ status: 500 });
    },
  });
  await assert.rejects(
    () =>
      client.submitOfflineSettlement({
        authority: FIXTURE_AUTHORITY_ID,
        privateKey: "ed25519:deadbeef",
        transfer: { bundle_id: "deadbeef", receipts: [] },
        buildClaimOverrides: [{ appId: "com.example.android" }],
      }),
    /tx_id_hex/i,
  );
  assert.equal(fetchCalled, false);
});

test("submitOfflineSettlementAndWait delegates to submit + waitForTransactionStatus", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 200 }),
  });
  const settlement = {
    bundle_id_hex: "deadbeef",
    transaction_hash_hex: "ca".repeat(32),
  };
  let submitArgs = null;
  let waitArgs = null;
  client.submitOfflineSettlement = async (request, options) => {
    submitArgs = { request, options };
    return settlement;
  };
  client.waitForTransactionStatus = async (hashHex, pollOptions) => {
    waitArgs = { hashHex, pollOptions };
    return {
      kind: "Transaction",
      content: { hash: hashHex, status: { kind: "Committed", content: null } },
    };
  };
  const controller = new AbortController();
  const request = {
    authority: FIXTURE_AUTHORITY_ID,
    privateKey: "ed25519:deadbeef",
    transfer: { bundle_id: "deadbeef", receipts: [] },
  };

  const response = await client.submitOfflineSettlementAndWait(request, {
    signal: controller.signal,
    intervalMs: 0,
    timeoutMs: null,
    maxAttempts: 4,
  });

  assert.equal(response, settlement);
  assert.ok(submitArgs, "submitOfflineSettlement should be called");
  assert.equal(submitArgs.request, request);
  assert.equal(submitArgs.options.signal, controller.signal);
  assert.ok(waitArgs, "waitForTransactionStatus should be called");
  assert.equal(waitArgs.hashHex, settlement.transaction_hash_hex);
  assert.equal(waitArgs.pollOptions.signal, controller.signal);
  assert.equal(waitArgs.pollOptions.intervalMs, 0);
  assert.equal(waitArgs.pollOptions.timeoutMs, null);
  assert.equal(waitArgs.pollOptions.maxAttempts, 4);
});

test("submitOfflineSettlementAndWait rejects when settlement response lacks tx hash", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 200 }),
  });
  client.submitOfflineSettlement = async () => ({
    bundle_id_hex: "deadbeef",
    transaction_hash_hex: null,
  });
  let waitCalled = false;
  client.waitForTransactionStatus = async () => {
    waitCalled = true;
    return null;
  };

  await assert.rejects(
    () =>
      client.submitOfflineSettlementAndWait({
        authority: FIXTURE_AUTHORITY_ID,
        privateKey: "ed25519:deadbeef",
        transfer: { bundle_id: "deadbeef", receipts: [] },
      }),
    /missing transaction_hash_hex/i,
  );
  assert.equal(waitCalled, false);
});

test("issueOfflineBuildClaim posts request and parses response", async () => {
  const certificateIdHex = "ab".repeat(32);
  const txIdHex = "cd".repeat(32);
  const claimIdHex = "ef".repeat(32);
  let capturedRequest = null;
  const fetchImpl = async (url, init) => {
    capturedRequest = { url, init };
    return createResponse({
      status: 200,
      jsonData: {
        claim_id_hex: claimIdHex,
        build_claim: {
          claim_id:
            "hash:FEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACE#CD31",
          nonce:
            "hash:ABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCD#C9C5",
          platform: "Apple",
          app_id: "com.example.ios",
          build_number: 77,
          issued_at_ms: 1_700_000_000_000,
          expires_at_ms: 1_700_000_100_000,
          operator_signature: "AA",
        },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const response = await client.issueOfflineBuildClaim({
    certificateIdHex,
    txIdHex,
    platform: "apple",
    appId: "com.example.ios",
    buildNumber: 77,
  });
  assert.ok(capturedRequest, "request not captured");
  assert.equal(new URL(capturedRequest.url).pathname, "/v1/offline/build-claims/issue");
  const body = JSON.parse(capturedRequest.init.body);
  assert.equal(body.certificate_id_hex, certificateIdHex);
  assert.equal(body.tx_id_hex, txIdHex);
  assert.equal(body.platform, "apple");
  assert.equal(body.app_id, "com.example.ios");
  assert.equal(body.build_number, 77);
  assert.equal(response.claim_id_hex, claimIdHex);
  assert.equal(
    response.build_claim.claim_id,
    "hash:FEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACE#CD31",
  );
  assert.equal(
    response.build_claim.nonce,
    "hash:ABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCD#C9C5",
  );
  assert.equal(response.build_claim.platform, "Apple");
  assert.equal(response.build_claim.app_id, "com.example.ios");
  assert.equal(response.build_claim.build_number, 77);
});

test("issueOfflineBuildClaim validates required ids before network call", async () => {
  let fetchCalled = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      fetchCalled = true;
      return createResponse({ status: 500 });
    },
  });
  await assert.rejects(
    () =>
      client.issueOfflineBuildClaim({
        certificateIdHex: "ab".repeat(32),
        platform: "apple",
      }),
    /tx_id_hex/i,
  );
  assert.equal(fetchCalled, false);
});

test("issueOfflineBuildClaim validates platform before network call", async () => {
  let fetchCalled = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      fetchCalled = true;
      return createResponse({ status: 500 });
    },
  });
  await assert.rejects(
    () =>
      client.issueOfflineBuildClaim({
        certificateIdHex: "ab".repeat(32),
        txIdHex: "cd".repeat(32),
        platform: "windows-phone",
      }),
    /platform/i,
  );
  assert.equal(fetchCalled, false);
});

test("issueOfflineBuildClaim rejects malformed build-claim response", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 200,
        jsonData: {
          claim_id_hex: "ef".repeat(32),
          build_claim: {
            claim_id:
              "hash:FEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACE#CD31",
            nonce:
              "hash:ABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCD#C9C5",
            platform: "windows-phone",
            app_id: "com.example.ios",
            build_number: 77,
            issued_at_ms: 1_700_000_000_000,
            expires_at_ms: 1_700_000_100_000,
            operator_signature: "AA",
          },
        },
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    () =>
      client.issueOfflineBuildClaim({
        certificateIdHex: "ab".repeat(32),
        txIdHex: "cd".repeat(32),
        platform: "apple",
      }),
    /build_claim\.platform/i,
  );
});

test("issueOfflineCertificate rejects invalid Numeric amounts", async () => {
  let fetchCalled = false;
  const fetchImpl = async () => {
    fetchCalled = true;
    return createResponse({ status: 500 });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const draft = {
    controller: FIXTURE_ALICE_ID,
    allowance: {
      asset: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
      amount: "1e-3",
      commitment: Buffer.from([1, 2, 3]),
    },
    spend_public_key: "ed0120deadbeef",
    attestation_report: new Uint8Array([4, 5, 6]),
    issued_at_ms: 100,
    expires_at_ms: 200,
    policy: {
      max_balance: "10",
      max_tx_value: "5",
      expires_at_ms: 200,
    },
  };
  await assert.rejects(
    () => client.issueOfflineCertificate(draft),
    /Numeric literal/i,
  );
  assert.equal(fetchCalled, false);
});

test("issueOfflineCertificateRenewal posts to renewal path", async () => {
  const certId = "deadbeef".repeat(8);
  let capturedRequest = null;
  const fetchImpl = async (url, init) => {
    capturedRequest = { url, init };
    return createResponse({
      status: 200,
      jsonData: {
        certificate_id_hex: certId,
        certificate: {
          controller: FIXTURE_ALICE_ID,
          operator: FIXTURE_AUTHORITY_ID,
          allowance: {
            asset: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
            amount: "10",
            commitment: [1, 2, 3],
          },
          spend_public_key: "ed0120deadbeef",
          attestation_report: [4, 5, 6],
          issued_at_ms: 100,
          expires_at_ms: 200,
          policy: {
            max_balance: "10",
            max_tx_value: "5",
            expires_at_ms: 200,
          },
          operator_signature: "AA",
          metadata: {},
          verdict_id: null,
          attestation_nonce: null,
          refresh_at_ms: null,
        },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await client.issueOfflineCertificateRenewal(certId.toUpperCase(), {
    controller: FIXTURE_ALICE_ID,
    allowance: {
      asset: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
      amount: "10",
      commitment: [1, 2, 3],
    },
    spend_public_key: "ed0120deadbeef",
    attestation_report: [4, 5, 6],
    issued_at_ms: 100,
    expires_at_ms: 200,
    policy: {
      max_balance: "10",
      max_tx_value: "5",
      expires_at_ms: 200,
    },
  });
  assert.ok(capturedRequest, "request not captured");
  const parsed = new URL(capturedRequest.url);
  assert.equal(
    parsed.pathname,
    `/v1/offline/certificates/${certId}/renew/issue`,
  );
  const body = JSON.parse(capturedRequest.init.body);
  assert.ok(body.certificate);
  assert.equal(FIXTURE_AUTHORITY_ID in body.certificate, false);
});

test("registerOfflineAllowance posts certificate and parses response", async () => {
  const certId = "deadbeef".repeat(8);
  let capturedRequest = null;
  const fetchImpl = async (url, init) => {
    capturedRequest = { url, init };
    return createResponse({
      status: 200,
      jsonData: {
        certificate_id_hex: certId.toUpperCase(),
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const certificate = {
    controller: FIXTURE_ALICE_ID,
    operator: FIXTURE_AUTHORITY_ID,
    allowance: {
      asset: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
      amount: "10",
      commitment: [1, 2, 3],
    },
    spend_public_key: "ed0120deadbeef",
    attestation_report: [4, 5, 6],
    issued_at_ms: 100,
    expires_at_ms: 200,
    policy: {
      max_balance: "10",
      max_tx_value: "5",
      expires_at_ms: 200,
    },
    operator_signature: "aa",
    metadata: {},
    verdict_id: null,
    attestation_nonce: null,
    refresh_at_ms: null,
  };
  const response = await client.registerOfflineAllowance({
    authority: FIXTURE_AUTHORITY_ID,
    privateKey: Buffer.alloc(32, 1),
    certificate,
  });
  assert.ok(capturedRequest, "request not captured");
  assert.equal(new URL(capturedRequest.url).pathname, "/v1/offline/allowances");
  const body = JSON.parse(capturedRequest.init.body);
  assert.equal(body.authority, FIXTURE_AUTHORITY_ID);
  assert.ok(body.private_key.startsWith("ed25519:"));
  assert.equal(body.certificate.operator_signature, "AA");
  assert.equal(response.certificate_id_hex, certId.toLowerCase());
});

test("renewOfflineAllowance posts to renewal path", async () => {
  const certId = "deadbeef".repeat(8);
  let capturedRequest = null;
  const fetchImpl = async (url, init) => {
    capturedRequest = { url, init };
    return createResponse({
      status: 200,
      jsonData: { certificate_id_hex: certId },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const certificate = {
    controller: FIXTURE_ALICE_ID,
    operator: FIXTURE_AUTHORITY_ID,
    allowance: {
      asset: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
      amount: "10",
      commitment: [1, 2, 3],
    },
    spend_public_key: "ed0120deadbeef",
    attestation_report: [4, 5, 6],
    issued_at_ms: 100,
    expires_at_ms: 200,
    policy: {
      max_balance: "10",
      max_tx_value: "5",
      expires_at_ms: 200,
    },
    operator_signature: "AA",
    metadata: {},
    verdict_id: null,
    attestation_nonce: null,
    refresh_at_ms: null,
  };
  await client.renewOfflineAllowance(certId.toUpperCase(), {
    authority: FIXTURE_AUTHORITY_ID,
    privateKey: "ed25519:deadbeef",
    certificate,
  });
  assert.ok(capturedRequest, "request not captured");
  const parsed = new URL(capturedRequest.url);
  assert.equal(
    parsed.pathname,
    `/v1/offline/allowances/${certId}/renew`,
  );
  const body = JSON.parse(capturedRequest.init.body);
  assert.ok(body.certificate);
});

test("topUpOfflineAllowance chains issue and register", async () => {
  const certId = "deadbeef".repeat(8);
  const requests = [];
  const responses = [
    createResponse({
      status: 200,
      jsonData: {
        certificate_id_hex: certId,
        certificate: {
          controller: FIXTURE_ALICE_ID,
          operator: FIXTURE_AUTHORITY_ID,
          allowance: {
            asset: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
            amount: "10",
            commitment: [1, 2, 3],
          },
          spend_public_key: "ed0120deadbeef",
          attestation_report: [4, 5, 6],
          issued_at_ms: 100,
          expires_at_ms: 200,
          policy: {
            max_balance: "10",
            max_tx_value: "5",
            expires_at_ms: 200,
          },
          operator_signature: "bb",
          metadata: {},
          verdict_id: null,
          attestation_nonce: null,
          refresh_at_ms: null,
        },
      },
      headers: { "content-type": "application/json" },
    }),
    createResponse({
      status: 200,
      jsonData: { certificate_id_hex: certId.toUpperCase() },
      headers: { "content-type": "application/json" },
    }),
  ];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return responses.shift();
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const draft = {
    controller: FIXTURE_ALICE_ID,
    allowance: {
      asset: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
      amount: "10",
      commitment: Buffer.from([1, 2, 3]),
    },
    spend_public_key: "ed0120deadbeef",
    attestation_report: new Uint8Array([4, 5, 6]),
    issued_at_ms: 100,
    expires_at_ms: 200,
    policy: {
      max_balance: "10",
      max_tx_value: "5",
      expires_at_ms: 200,
    },
  };
  const response = await client.topUpOfflineAllowance({
    authority: FIXTURE_AUTHORITY_ID,
    privateKey: Buffer.alloc(32, 2),
    certificate: draft,
  });
  assert.equal(response.certificate.certificate_id_hex, certId);
  assert.equal(response.registration.certificate_id_hex, certId);
  assert.equal(requests.length, 2);
  assert.equal(new URL(requests[0].url).pathname, "/v1/offline/certificates/issue");
  const issueBody = JSON.parse(requests[0].init.body);
  assert.equal(FIXTURE_AUTHORITY_ID in issueBody.certificate, false);
  assert.equal(new URL(requests[1].url).pathname, "/v1/offline/allowances");
  const registerBody = JSON.parse(requests[1].init.body);
  assert.ok(registerBody.private_key.startsWith("ed25519:"));
  assert.equal(registerBody.certificate.operator_signature, "BB");
});

test("topUpOfflineAllowanceRenewal chains issue and renew", async () => {
  const certId = "deadbeef".repeat(8);
  const requests = [];
  const responses = [
    createResponse({
      status: 200,
      jsonData: {
        certificate_id_hex: certId,
        certificate: {
          controller: FIXTURE_ALICE_ID,
          operator: FIXTURE_AUTHORITY_ID,
          allowance: {
            asset: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
            amount: "10",
            commitment: [1, 2, 3],
          },
          spend_public_key: "ed0120deadbeef",
          attestation_report: [4, 5, 6],
          issued_at_ms: 100,
          expires_at_ms: 200,
          policy: {
            max_balance: "10",
            max_tx_value: "5",
            expires_at_ms: 200,
          },
          operator_signature: "CC",
          metadata: {},
          verdict_id: null,
          attestation_nonce: null,
          refresh_at_ms: null,
        },
      },
      headers: { "content-type": "application/json" },
    }),
    createResponse({
      status: 200,
      jsonData: { certificate_id_hex: certId },
      headers: { "content-type": "application/json" },
    }),
  ];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return responses.shift();
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const draft = {
    controller: FIXTURE_ALICE_ID,
    allowance: {
      asset: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
      amount: "10",
      commitment: Buffer.from([1, 2, 3]),
    },
    spend_public_key: "ed0120deadbeef",
    attestation_report: new Uint8Array([4, 5, 6]),
    issued_at_ms: 100,
    expires_at_ms: 200,
    policy: {
      max_balance: "10",
      max_tx_value: "5",
      expires_at_ms: 200,
    },
  };
  await client.topUpOfflineAllowanceRenewal(certId.toUpperCase(), {
    authority: FIXTURE_AUTHORITY_ID,
    privateKey: "ed25519:deadbeef",
    certificate: draft,
  });
  assert.equal(requests.length, 2);
  assert.equal(
    new URL(requests[0].url).pathname,
    `/v1/offline/certificates/${certId}/renew/issue`,
  );
  const renewIssueBody = JSON.parse(requests[0].init.body);
  assert.equal(FIXTURE_AUTHORITY_ID in renewIssueBody.certificate, false);
  assert.equal(
    new URL(requests[1].url).pathname,
    `/v1/offline/allowances/${certId}/renew`,
  );
});

test("listOfflineAllowances encodes convenience query params", async () => {
  let capturedUrl = null;
  const assetId = "norito:DEADBEEF";
  const normalizedAssetId = "norito:deadbeef";
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url) => {
      capturedUrl = url;
      return createResponse({
        status: 200,
        jsonData: { items: [], total: 0 },
        headers: { "content-type": "application/json" },
      });
    },
  });
  await client.listOfflineAllowances({
    controllerId: FIXTURE_ALICE_ID,
    assetId,
    certificateExpiresBeforeMs: 1_000,
    certificateExpiresAfterMs: 100,
    policyExpiresBeforeMs: 2_000,
    policyExpiresAfterMs: 250,
    refreshBeforeMs: 750,
    refreshAfterMs: 500,
    attestationNonceHex: "CAFEBABE",
    verdictIdHex: "DEADBEEF",
    requireVerdict: true,
  });
  assert.ok(capturedUrl, "request not issued");
  const parsed = new URL(capturedUrl);
  assert.equal(parsed.searchParams.get("controller_id"), FIXTURE_ALICE_ID);
  assert.equal(parsed.searchParams.get("asset_id"), normalizedAssetId);
  assert.equal(parsed.searchParams.get("certificate_expires_before_ms"), "1000");
  assert.equal(parsed.searchParams.get("certificate_expires_after_ms"), "100");
  assert.equal(parsed.searchParams.get("policy_expires_before_ms"), "2000");
  assert.equal(parsed.searchParams.get("policy_expires_after_ms"), "250");
  assert.equal(parsed.searchParams.get("refresh_before_ms"), "750");
  assert.equal(parsed.searchParams.get("refresh_after_ms"), "500");
  assert.equal(parsed.searchParams.get("attestation_nonce_hex"), "cafebabe");
  assert.equal(parsed.searchParams.get("verdict_id_hex"), "deadbeef");
  assert.equal(parsed.searchParams.get("require_verdict"), "true");
  assert.equal(parsed.searchParams.get("only_missing_verdict"), null);
});

test("listOfflineAllowances rejects conflicting verdict filters", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not fetch");
    },
  });
  await assert.rejects(
    () =>
      client.listOfflineAllowances({
        verdictIdHex: "deadbeef",
        onlyMissingVerdict: true,
      }),
    (error) =>
      expectValidationErrorFixture(
        error,
        "listOfflineAllowances_conflict_verdictId_onlyMissingVerdict",
      ),
  );
  await assert.rejects(
    () =>
      client.listOfflineAllowances({
        requireVerdict: true,
        onlyMissingVerdict: true,
      }),
    (error) =>
      expectValidationErrorFixture(
        error,
        "listOfflineAllowances_conflict_requireVerdict_onlyMissingVerdict",
      ),
  );
});

test("listOfflineAllowances rejects unsupported filter fields", async () => {
  let invoked = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      invoked = true;
      throw new Error("network should not be invoked");
    },
  });
  await assert.rejects(
    client.listOfflineAllowances({
      filter: { Eq: ["unsupported_field", "value"] },
    }),
    /"unsupported_field" is not allowed/i,
  );
  assert.equal(invoked, false);
});

test("listOfflineAllowances rejects non-boolean verdict toggles", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not fetch");
    },
  });
  await assert.rejects(
    () =>
      client.listOfflineAllowances({
        requireVerdict: "yes",
      }),
    (error) => expectValidationErrorFixture(error, "listOfflineAllowances_requireVerdict_type"),
  );
});

test("listOfflineAllowances accepts verdict/expiry filter fields", async () => {
  let capturedFilter = null;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url) => {
      const parsed = new URL(url);
      capturedFilter = parsed.searchParams.get("filter");
      return createResponse({
        status: 200,
        jsonData: { items: [], total: 0 },
        headers: { "content-type": "application/json" },
      });
    },
  });
  const filter = {
    And: [
      { Eq: ["verdict_id_hex", "deadbeef"] },
      { Exists: "attestation_nonce_hex" },
      { Lt: ["certificate_expires_at_ms", 9_000] },
      { Gt: ["refresh_at_ms", 1_000] },
    ],
  };
  await client.listOfflineAllowances({ filter, limit: 1 });
  assert.equal(capturedFilter, JSON.stringify(filter));
});

test("listOfflineAllowances rejects invalid signal values", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listOfflineAllowances({ signal: {} }),
    /options for \/v1\/offline\/allowances options\.signal must be an AbortSignal/,
  );
});

test("listOfflineAllowances validates controllerId literals", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not fetch");
    },
  });
  await assert.rejects(
    () => client.listOfflineAllowances({ controllerId: 123 }),
    /controllerId/i,
  );
});

test("queryOfflineAllowances forwards convenience options to query endpoint", async () => {
  let capturedUrl = null;
  let capturedBody = null;
  const fetchImpl = async (url, init) => {
    capturedUrl = url;
    capturedBody = init?.body ?? null;
    return createResponse({
      status: 200,
      jsonData: { items: [], total: 0 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const filter = { Eq: ["controller_id", FIXTURE_ALICE_ID] };
  await client.queryOfflineAllowances({
    controllerId: FIXTURE_ALICE_ID,
    receiverId: FIXTURE_BOB_ID,
    certificateExpiresBeforeMs: "5000",
    verdictIdHex: "DEADBEEF",
    attestationNonceHex: "CAFEBABE",
    platformPolicy: "HARDENED",
    includeExpired: true,
    fetchSize: 2,
    queryName: "offline_allowances",
    filter,
  });
  const parsed = new URL(capturedUrl);
  assert.equal(parsed.pathname, "/v1/offline/allowances/query");
  assert.equal(parsed.searchParams.get("controller_id"), FIXTURE_ALICE_ID);
  assert.equal(parsed.searchParams.get("receiver_id"), FIXTURE_BOB_ID);
  assert.equal(parsed.searchParams.get("certificate_expires_before_ms"), "5000");
  assert.equal(parsed.searchParams.get("verdict_id_hex"), "deadbeef");
  assert.equal(parsed.searchParams.get("attestation_nonce_hex"), "cafebabe");
  assert.equal(parsed.searchParams.get("platform_policy"), "hardened");
  assert.equal(parsed.searchParams.get("include_expired"), "true");
  assert.equal(parsed.searchParams.get("canonical_i105"), null);
  const body = JSON.parse(capturedBody);
  assert.deepEqual(body.filter, filter);
  assert.equal(body.fetch_size, 2);
  assert.equal(body.query, "offline_allowances");
});

test("queryOfflineAllowances rejects unsupported filter fields", async () => {
  let invoked = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      invoked = true;
      throw new Error("network should not be invoked");
    },
  });
  await assert.rejects(
    client.queryOfflineAllowances({
      filter: { Eq: ["unsupported_field", "value"] },
    }),
    /"unsupported_field" is not allowed/i,
  );
  assert.equal(invoked, false);
});

test("listOfflineSummaries rejects invalid signal values", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listOfflineSummaries({ signal: {} }),
    /options for \/v1\/offline\/summaries options\.signal must be an AbortSignal/,
  );
});

test("listOfflineRevocations normalizes payloads and query params", async () => {
  let capturedUrl = null;
  const revocationRecord = { verdict_id_hex: "deadbeef", metadata: { foo: "bar" } };
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: {
        items: [
          {
            verdict_id_hex: "DEADBEEF",
            issuer_id: FIXTURE_ISSUER_ID,
            issuer_display: "soraqqqqqqqq",
            revoked_at_ms: "1234",
            reason: "compromised_device",
            note: "rotated",
            metadata: { foo: "bar" },
            record: revocationRecord,
          },
        ],
        total: "1",
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const filter = { Eq: ["reason", "compromised_device"] };
  const page = await client.listOfflineRevocations({
    filter,
    limit: 5,
    offset: 2,
    sort: "revoked_at_ms:desc",
  });
  assert.ok(capturedUrl, "request not issued");
  const parsed = new URL(capturedUrl);
  assert.equal(parsed.pathname, "/v1/offline/revocations");
  assert.equal(parsed.searchParams.get("limit"), "5");
  assert.equal(parsed.searchParams.get("offset"), "2");
  assert.equal(parsed.searchParams.get("sort"), "revoked_at_ms:desc");
  assert.equal(parsed.searchParams.get("filter"), JSON.stringify(filter));
  assert.equal(page.total, 1);
  assert.equal(page.items.length, 1);
  const item = page.items[0];
  assert.equal(item.verdict_id_hex, "deadbeef");
  assert.equal(item.issuer_id, FIXTURE_ISSUER_ID);
  assert.equal(item.issuer_display, "soraqqqqqqqq");
  assert.equal(item.revoked_at_ms, 1234);
  assert.equal(item.reason, "compromised_device");
  assert.equal(item.note, "rotated");
  assert.deepEqual(item.metadata, { foo: "bar" });
  assert.deepEqual(item.record, revocationRecord);
});

test("listOfflineRevocations rejects unsupported filter fields", async () => {
  let invoked = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      invoked = true;
      throw new Error("network should not be invoked");
    },
  });
  await assert.rejects(
    client.listOfflineRevocations({
      filter: { Eq: ["unsupported_field", "value"] },
    }),
    /"unsupported_field" is not allowed/i,
  );
  assert.equal(invoked, false);
});

test("queryOfflineRevocations validates filter envelopes", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("should not reach network");
    },
  });
  await assert.rejects(
    () =>
      client.queryOfflineRevocations({
        filter: { Gt: ["issuer_id", 10] },
      }),
    /does not support range filters/i,
  );
});

test("queryOfflineSummaries rejects invalid signal values", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.queryOfflineSummaries({ signal: {} }),
    /options for \/v1\/offline\/summaries\/query options\.signal must be an AbortSignal/,
  );
});

test("queryOfflineTransfers rejects invalid signal values", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.queryOfflineTransfers({ signal: {} }),
    /options for \/v1\/offline\/transfers\/query options\.signal must be an AbortSignal/,
  );
});

test("listOfflineSummaries normalizes counters and query params", async () => {
  let capturedUrl;
  const payload = {
    items: [
      {
        certificate_id_hex: "feedface",
        controller_id: FIXTURE_ALICE_ID,
        controller_display: "soraalice",
        summary_hash_hex: "beadbead",
        apple_key_counters: { primary: "4" },
        android_series_counters: null,
        policy_key_counters: { policy: 2 },
        counter_totals: {
          total_counters: "3",
          total_weight: "9",
          apple: "1",
          android: "1",
          policy: "1",
        },
        metadata: { cohort: "alpha" },
      },
    ],
    total: "1",
  };
  const fetchImpl = async (url) => {
    capturedUrl = url;
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const filter = { Eq: ["controller_id", FIXTURE_ALICE_ID] };
  const page = await client.listOfflineSummaries({
    limit: 10,
    offset: 5,
    filter,
    sort: [{ key: "total_counters", order: "desc" }],
  });
  assert.ok(capturedUrl);
  const parsed = new URL(capturedUrl);
  assert.equal(parsed.pathname, "/v1/offline/summaries");
  assert.equal(parsed.searchParams.get("limit"), "10");
  assert.equal(parsed.searchParams.get("offset"), "5");
  assert.equal(parsed.searchParams.get("filter"), JSON.stringify(filter));
  assert.equal(parsed.searchParams.get("sort"), "total_counters:desc");
  assert.equal(parsed.searchParams.get("canonical_i105"), null);
  assert.equal(page.total, 1);
  assert.equal(page.items.length, 1);
  const item = page.items[0];
  assert.equal(item.certificate_id_hex, "feedface");
  assert.equal(item.controller_id, FIXTURE_ALICE_ID);
  assert.equal(item.controller_display, "soraalice");
  assert.equal(item.summary_hash_hex, "beadbead");
  assert.deepEqual(item.apple_key_counters, { primary: 4 });
  assert.deepEqual(item.android_series_counters, {});
  assert.deepEqual(item.policy_key_counters, { policy: 2 });
  assert.deepEqual(item.counter_totals, {
    total_counters: 3,
    total_weight: 9,
    apple: 1,
    android: 1,
    policy: 1,
  });
  assert.deepEqual(item.metadata, { cohort: "alpha" });
});

test("queryOfflineSummaries posts envelope and normalizes payload", async () => {
  let capturedRequest;
  const payload = {
    items: [
      {
        certificate_id_hex: "cafebabe",
        controller_id: FIXTURE_BOB_ID,
        controller_display: "sorabob",
        summary_hash_hex: "abbaabba",
        apple_key_counters: { regional: "7" },
        android_series_counters: { fallback: 2 },
      },
    ],
  };
  const fetchImpl = async (url, init = {}) => {
    capturedRequest = { url, init };
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const filter = { Exists: "controller_id" };
  const select = [{ fields: ["controller_id"] }];
  const page = await client.queryOfflineSummaries({
    fetchSize: 25,
    limit: 2,
    offset: 4,
    filter,
    sort: "controller_id:asc",
    queryName: "summaryCountersByController",
    select,
  });
  assert.ok(capturedRequest);
  assert.equal(capturedRequest.url, `${BASE_URL}/v1/offline/summaries/query`);
  assert.equal(capturedRequest.init.method, "POST");
  assert.equal(capturedRequest.init.headers["Content-Type"], "application/json");
  const envelope = JSON.parse(capturedRequest.init.body);
  assert.deepEqual(envelope.pagination, { offset: 4, limit: 2 });
  assert.equal(envelope.fetch_size, 25);
  assert.deepEqual(envelope.filter, filter);
  assert.deepEqual(envelope.sort, [{ key: "controller_id", order: "asc" }]);
  assert.equal(envelope.canonical_i105, undefined);
  assert.equal(envelope.query, "summaryCountersByController");
  assert.deepEqual(envelope.select, select);
  assert.equal(page.total, 1);
  const item = page.items[0];
  assert.equal(item.certificate_id_hex, "cafebabe");
  assert.equal(item.controller_id, FIXTURE_BOB_ID);
  assert.equal(item.summary_hash_hex, "abbaabba");
  assert.deepEqual(item.apple_key_counters, { regional: 7 });
  assert.deepEqual(item.android_series_counters, { fallback: 2 });
});

test("listOfflineSummaries rejects unsupported filter fields", async () => {
  let invoked = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      invoked = true;
      throw new Error("should not reach Torii");
    },
  });
  await assert.rejects(
    () =>
      client.listOfflineSummaries({
        filter: { Eq: ["summary_hash_hex", "beadbead"] },
      }),
    /"summary_hash_hex" is not allowed/i,
  );
  assert.equal(invoked, false);
});

test("queryOfflineSummaries rejects unsupported filter fields", async () => {
  let invoked = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      invoked = true;
      throw new Error("should not reach Torii");
    },
  });
  await assert.rejects(
    () =>
      client.queryOfflineSummaries({
        filter: { Eq: ["asset_id", "7EAD8EFYUx1aVKZPUU1fyKvr8dF1"] },
      }),
    /"asset_id" is not allowed/i,
  );
  assert.equal(invoked, false);
});

test("queryOfflineTransfers rejects range operators", async () => {
  let invoked = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      invoked = true;
      throw new Error("network should not be invoked");
    },
  });
  await assert.rejects(
    client.queryOfflineTransfers({
      filter: { Lt: ["bundle_id_hex", 1] },
    }),
    /does not support range filters/i,
  );
  assert.equal(invoked, false);
});

test("queryOfflineTransfers accepts certificate and verdict filters", async () => {
  let capturedRequest;
  const fetchImpl = async (_url, init) => {
    capturedRequest = { init };
    return createResponse({
      status: 200,
      jsonData: createOfflineTransferPayload(),
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const filter = {
    And: [
      { Eq: ["certificate_id_hex", "cafebabe"] },
      { In: ["verdict_id_hex", ["deadbeef", "feedface"]] },
      { Eq: ["attestation_nonce_hex", "beadbead"] },
    ],
  };
  await client.queryOfflineTransfers({ filter });
  assert.ok(capturedRequest);
  const envelope = JSON.parse(capturedRequest.init.body);
  assert.deepEqual(envelope.filter, filter);
});

test("queryOfflineTransfers allows numeric range filters", async () => {
  let capturedRequest;
  const fetchImpl = async (_url, init) => {
    capturedRequest = { init };
    return createResponse({
      status: 200,
      jsonData: createOfflineTransferPayload(),
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const filter = {
    And: [
      { Gte: ["recorded_at_ms", 1_000] },
      { Lt: ["recorded_at_height", 10_000] },
      { Exists: "certificate_expires_at_ms" },
      { Gt: ["receipt_count", 0] },
    ],
  };
  await client.queryOfflineTransfers({ filter });
  assert.ok(capturedRequest);
  const envelope = JSON.parse(capturedRequest.init.body);
  assert.deepEqual(envelope.filter, filter);
});

test("queryOfflineTransfers posts envelope and normalizes optional fields", async () => {
  let capturedInit;
  const expectedReceiverId = normalizeAccountId(
    FIXTURE_BOB_ID,
    "queryOfflineTransfers.receiver_id",
  );
  const expectedDepositAccountId = normalizeAccountId(
    FIXTURE_MERCHANT_ID,
    "queryOfflineTransfers.deposit_account_id",
  );
  const payload = {
    items: [
      {
        bundle_id_hex: "ff00",
        controller_id: FIXTURE_ALICE_ID,
        controller_display: "soracontroller",
        receiver_id: FIXTURE_BOB_ID,
        receiver_display: "sorareceiver",
        deposit_account_id: FIXTURE_MERCHANT_ID,
        deposit_account_display: "soravault",
        status: "Pending",
        recorded_at_ms: 1_000,
        recorded_at_height: 5,
        archived_at_height: null,
        certificate_id_hex: "ab".repeat(32),
        certificate_expires_at_ms: 2_000,
        policy_expires_at_ms: 3_000,
        refresh_at_ms: 4_000,
        verdict_id_hex: "cd".repeat(32),
        attestation_nonce_hex: "ef".repeat(32),
        status_transitions: [],
        receipt_count: "2",
        total_amount: "42",
        claimed_delta: "17",
        platform_policy: "provisioned",
        platform_token_snapshot: {
          policy: "provisioned",
          attestation_jws_b64: "eyJhbGciOiJFZERTQSJ9",
        },
        transfer: {
          receipts: [],
          balance_proof: { claimed_delta: "17" },
          metadata: {
            "android.integrity.policy": "provisioned",
            "android.provisioned.inspector_public_key": "inspector",
            "android.provisioned.manifest_schema": "schema-v1",
            "android.provisioned.manifest_version": 1,
            "android.provisioned.max_manifest_age_ms": 60_000,
            "android.provisioned.manifest_digest": "ab".repeat(32),
          },
        },
      },
    ],
    total: 1,
  };
  const fetchImpl = async (_url, init) => {
    capturedInit = init;
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.queryOfflineTransfers({
    filter: { Eq: ["asset_id", "7EAD8EFYUx1aVKZPUU1fyKvr8dF1"] },
    fetchSize: 25,
    limit: 5,
    sort: [{ key: "receipt_count", order: "desc" }],
  });
  assert.ok(capturedInit);
  assert.equal(capturedInit.method, "POST");
  const body = JSON.parse(capturedInit.body);
  assert.deepEqual(body.pagination, { offset: 0, limit: 5 });
  assert.equal(body.fetch_size, 25);
  assert.equal(body.canonical_i105, undefined);
  assert.deepEqual(body.filter, { Eq: ["asset_id", "7EAD8EFYUx1aVKZPUU1fyKvr8dF1"] });
  assert.deepEqual(body.sort, [{ key: "receipt_count", order: "desc" }]);
  assert.equal(result.total, 1);
  const transfer = result.items[0];
  assert.equal(transfer.bundle_id_hex, "ff00");
  assert.equal(transfer.receiver_id, expectedReceiverId);
  assert.equal(transfer.deposit_account_id, expectedDepositAccountId);
  assert.equal(transfer.asset_id, null);
  assert.equal(transfer.receipt_count, 2);
  assert.equal(transfer.total_amount, "42");
  assert.equal(transfer.claimed_delta, "17");
  assert.equal(transfer.platform_policy, "provisioned");
  assert.deepEqual(transfer.platform_token_snapshot, {
    policy: "provisioned",
    attestation_jws_b64: "eyJhbGciOiJFZERTQSJ9",
  });
  assert.deepEqual(transfer.transfer, payload.items[0].transfer);
});

test("iterateOfflineAllowances paginates and honours maxItems", async () => {
  const allowances = [
    createOfflineAllowanceItem({ certificate_id_hex: "aa".repeat(32) }),
    createOfflineAllowanceItem({
      certificate_id_hex: "bb".repeat(32),
      controller_id: SAMPLE_ACCOUNT_FORMS.i105,
    }),
    createOfflineAllowanceItem({
      certificate_id_hex: "cc".repeat(32),
      controller_id: SAMPLE_ACCOUNT_FORMS.i105Default,
    }),
    createOfflineAllowanceItem({
      certificate_id_hex: "dd".repeat(32),
      controller_id: SAMPLE_ACCOUNT_ID,
    }),
  ];
  const capturedRequests = [];
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/offline/allowances");
    const limit = Number(parsed.searchParams.get("limit"));
    const offset = Number(parsed.searchParams.get("offset") ?? "0");
    capturedRequests.push({ limit, offset });
    const items = allowances.slice(offset, offset + limit);
    return createResponse({
      status: 200,
      jsonData: { items, total: allowances.length },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const certificates = [];
  for await (const allowance of client.iterateOfflineAllowances({
    pageSize: 2,
    maxItems: 3,
  })) {
    certificates.push(allowance.certificate_id_hex);
  }
  assert.deepEqual(certificates, allowances.slice(0, 3).map((entry) => entry.certificate_id_hex));
  assert.deepEqual(capturedRequests, [
    { limit: 2, offset: 0 },
    { limit: 1, offset: 2 },
  ]);
});

test("iterateOfflineSummariesQuery paginates structured responses", async () => {
  const summaries = [
    createOfflineSummaryItem({ summary_hash_hex: "11".repeat(32) }),
    createOfflineSummaryItem({
      summary_hash_hex: "22".repeat(32),
      controller_id: SAMPLE_ACCOUNT_FORMS.i105,
      controller_display: SAMPLE_ACCOUNT_FORMS.i105Default,
    }),
  ];
  const capturedRequests = [];
  const fetchImpl = async (url, init) => {
    assert.equal(new URL(url).pathname, "/v1/offline/summaries/query");
    const body = JSON.parse(init.body);
    const limit = Number(body.pagination?.limit ?? 0);
    const offset = Number(body.pagination?.offset ?? 0);
    capturedRequests.push({ limit, offset });
    const items = summaries.slice(offset, offset + limit);
    return createResponse({
      status: 200,
      jsonData: { items, total: summaries.length },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const hashes = [];
  for await (const summary of client.iterateOfflineSummariesQuery({ pageSize: 1 })) {
    hashes.push(summary.summary_hash_hex);
  }
  assert.deepEqual(hashes, summaries.map((entry) => entry.summary_hash_hex));
  assert.deepEqual(capturedRequests, [
    { limit: 1, offset: 0 },
    { limit: 1, offset: 1 },
  ]);
});

test("iterateOfflineTransfersQuery walks paginated transfer responses", async () => {
  const baseTransfer = cloneFixture(createOfflineTransferPayload().items[0]);
  const transfers = [
    {
      ...cloneFixture(baseTransfer),
      bundle_id_hex: "aa00",
      recorded_at_height: 1,
      controller_id: SAMPLE_ACCOUNT_ID,
      receiver_id: SAMPLE_ACCOUNT_FORMS.i105,
      receiver_display: SAMPLE_ACCOUNT_FORMS.i105Default,
      deposit_account_id: SAMPLE_ACCOUNT_FORMS.i105Default,
      deposit_account_display: SAMPLE_ACCOUNT_FORMS.i105Default,
    },
    {
      ...cloneFixture(baseTransfer),
      bundle_id_hex: "bb00",
      recorded_at_height: 2,
      controller_id: SAMPLE_ACCOUNT_FORMS.i105,
      receiver_id: SAMPLE_ACCOUNT_FORMS.canonical,
      receiver_display: SAMPLE_ACCOUNT_FORMS.i105Default,
      deposit_account_id: SAMPLE_ACCOUNT_FORMS.canonical,
      deposit_account_display: SAMPLE_ACCOUNT_FORMS.i105Default,
    },
    {
      ...cloneFixture(baseTransfer),
      bundle_id_hex: "cc00",
      recorded_at_height: 3,
      controller_id: SAMPLE_ACCOUNT_FORMS.i105Default,
      receiver_id: SAMPLE_ACCOUNT_ID,
      receiver_display: SAMPLE_ACCOUNT_FORMS.i105Default,
      deposit_account_id: SAMPLE_ACCOUNT_ID,
      deposit_account_display: SAMPLE_ACCOUNT_FORMS.i105Default,
    },
  ];
  const capturedRequests = [];
  const fetchImpl = async (url, init) => {
    assert.equal(new URL(url).pathname, "/v1/offline/transfers/query");
    const body = JSON.parse(init.body);
    const limit = Number(body.pagination?.limit ?? 0);
    const offset = Number(body.pagination?.offset ?? 0);
    capturedRequests.push({ limit, offset });
    const items = transfers.slice(offset, offset + limit);
    return createResponse({
      status: 200,
      jsonData: { items, total: transfers.length },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const bundleIds = [];
  for await (const transfer of client.iterateOfflineTransfersQuery({ pageSize: 2 })) {
    bundleIds.push(transfer.bundle_id_hex);
  }
  assert.deepEqual(bundleIds, transfers.map((entry) => entry.bundle_id_hex));
  assert.deepEqual(capturedRequests, [
    { limit: 2, offset: 0 },
    { limit: 2, offset: 2 },
  ]);
});

test("iterateOfflineRevocations paginates revocation lists", async () => {
  const revocations = [
    createOfflineRevocationItem({ verdict_id_hex: "DEAD".repeat(8) }),
    createOfflineRevocationItem({ verdict_id_hex: "feed".repeat(8) }),
  ];
  const capturedRequests = [];
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/offline/revocations");
    const limit = Number(parsed.searchParams.get("limit"));
    const offset = Number(parsed.searchParams.get("offset") ?? "0");
    capturedRequests.push({ limit, offset });
    const items = revocations.slice(offset, offset + limit);
    return createResponse({
      status: 200,
      jsonData: { items, total: revocations.length },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const verdicts = [];
  for await (const revocation of client.iterateOfflineRevocations({ pageSize: 1 })) {
    verdicts.push(revocation.verdict_id_hex);
  }
  assert.deepEqual(verdicts, revocations.map((entry) => entry.verdict_id_hex.toLowerCase()));
  assert.deepEqual(capturedRequests, [
    { limit: 1, offset: 0 },
    { limit: 1, offset: 1 },
  ]);
});

test("getOfflineRejectionStats normalizes payload and supports telemetry profile header", async () => {
  let capturedRequest;
  const fetchImpl = async (url, init = {}) => {
    capturedRequest = { url, init };
    return createResponse({
      status: 200,
      jsonData: {
        total: "3",
        items: [
          { platform: "apple", reason: "platform_attestation_invalid", count: "2" },
          { platform: "android", reason: "counter_violation", count: 1 },
        ],
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const snapshot = await client.getOfflineRejectionStats({
    telemetryProfile: "full",
    signal: undefined,
  });
  assert.ok(capturedRequest);
  assert.equal(capturedRequest.url, `${BASE_URL}/v1/offline/rejections`);
  assert.equal(capturedRequest.init.method, "GET");
  assert.equal(capturedRequest.init.headers.Accept, "application/json");
  assert.equal(capturedRequest.init.headers["X-Torii-Telemetry-Profile"], "full");
  assert.deepEqual(snapshot, {
    total: 3,
    items: [
      { platform: "apple", reason: "platform_attestation_invalid", count: 2 },
      { platform: "android", reason: "counter_violation", count: 1 },
    ],
  });
});

test("getOfflineRejectionStats tolerates telemetry gating and validates options", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 403,
      jsonData: {},
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const disabled = await client.getOfflineRejectionStats();
  assert.equal(disabled, null);
  await assert.rejects(
    () => client.getOfflineRejectionStats(123),
    /getOfflineRejectionStats options must be an object/,
  );
  await assert.rejects(
    () => client.getOfflineRejectionStats({ telemetryProfile: "" }),
    /telemetryProfile must not be empty/,
  );
  await assert.rejects(
    () => client.getOfflineRejectionStats({ telemetryProfile: "full", extra: true }),
    /getOfflineRejectionStats options contains unsupported fields: extra/,
  );
});

test("deleteTrigger tolerates missing records", async () => {
  const fetchImpl = async () => createResponse({ status: 404 });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.deleteTrigger("apps::obsolete");
  assert.equal(result, null);
});

test("deleteTrigger validates options before issuing request", async () => {
  const fetchImpl = async () => {
    throw new Error("should not fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.deleteTrigger("apps::obsolete", 123),
    /deleteTrigger options must be an object/,
  );
  await assert.rejects(
    () => client.deleteTrigger("apps::obsolete", { signal: {} }),
    /deleteTrigger options\.signal must be an AbortSignal/,
  );
  await assert.rejects(
    () => client.deleteTrigger("apps::obsolete", { memo: "nope" }),
    /deleteTrigger options contains unsupported fields: memo/,
  );
});

test("listPeers hits Torii endpoint and returns raw payload", async () => {
  let capturedRequest;
  const sample = [
    { address: "10.0.0.4:1337", id: { public_key: "ab".repeat(32) } },
  ];
  const fetchImpl = async (url, init) => {
    capturedRequest = { url, init };
    return createResponse({
      status: 200,
      jsonData: sample,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const peers = await client.listPeers();
  assert.equal(capturedRequest.url, `${BASE_URL}/v1/peers`);
  assert.equal(capturedRequest.init.method, "GET");
  assert.equal(capturedRequest.init.headers.Accept, "application/json");
  assert.deepEqual(peers, sample);
});

test("listPeers enforces options object", async () => {
  const fetchImpl = async () => {
    throw new Error("unexpected fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listPeers(null),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(error.message, /listPeers options must be an object/);
      return true;
    },
  );
});

test("listPeers rejects unsupported option fields", async () => {
  const fetchImpl = async () => {
    throw new Error("unexpected fetch");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listPeers({ signal: undefined, unexpected: true }),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(
        error.message,
        /listPeers options contains unsupported fields: unexpected/,
      );
      return true;
    },
  );
});

test("listPeersTyped normalizes address and public key", async () => {
  const payload = [
    {
      address: "  192.168.1.12:8080 ",
      id: { public_key: `0X${"AA".repeat(32)}` },
    },
  ];
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const peers = await client.listPeersTyped();
  assert.deepEqual(peers, [
    {
      address: "192.168.1.12:8080",
      public_key_hex: "aa".repeat(32),
    },
  ]);
});

test("listPeersTyped rejects malformed entries", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: [{}],
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listPeersTyped(),
    /peer list response\[0\]\.address/,
  );
});

test("getExplorerMetrics normalizes payload and tolerates telemetry gating", async () => {
  let callCount = 0;
  const fetchImpl = async (url, init = {}) => {
    callCount += 1;
    if (callCount === 1) {
      assert.equal(url, `${BASE_URL}/v1/explorer/metrics`);
      assert.equal(init.method, "GET");
      assert.equal(init.headers.Accept, "application/json");
      return createResponse({
        status: 200,
        jsonData: {
          peers: "5",
          domains: 3,
          accounts: "9",
          assets: 12,
          transactions_accepted: 7,
          transactions_rejected: "2",
          block: 42,
          block_created_at: "2025-01-01T00:00:00Z",
          finalized_block: "41",
          avg_commit_time: { ms: 250 },
          avg_block_time: { ms: 500 },
        },
        headers: { "content-type": "application/json" },
      });
    }
    return createResponse({ status: 403, jsonData: {}, headers: { "content-type": "application/json" } });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const snapshot = await client.getExplorerMetrics();
  assert.deepEqual(snapshot, {
    peers: 5,
    domains: 3,
    accounts: 9,
    assets: 12,
    transactionsAccepted: 7,
    transactionsRejected: 2,
    blockHeight: 42,
    blockCreatedAt: "2025-01-01T00:00:00Z",
    finalizedBlockHeight: 41,
    averageCommitTimeMs: 250,
    averageBlockTimeMs: 500,
  });
  const disabled = await client.getExplorerMetrics();
  assert.equal(disabled, null);
});

test("getExplorerMetrics rejects non-object options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () => client.getExplorerMetrics(123),
    /getExplorerMetrics options must be an object/,
  );
});

test("getExplorerMetrics rejects unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () => client.getExplorerMetrics({ unexpected: "value" }),
    /getExplorerMetrics options contains unsupported fields: unexpected/,
  );
});

test("getExplorerAccountQr normalizes payloads", async () => {
  const accountId = FIXTURE_ALICE_ID;
  let callCount = 0;
  const fetchImpl = async (url, init = {}) => {
    callCount += 1;
    const requestUrl = new URL(url);
    assert.equal(init.method, "GET");
    assert.equal(init.headers.Accept, "application/json");
    assert.equal(
      requestUrl.pathname,
      `/v1/explorer/accounts/${encodeURIComponent(accountId)}/qr`,
    );
    if (callCount === 1) {
      assert.equal(requestUrl.search, "");
      return createResponse({
        status: 200,
        jsonData: {
          canonical_id: accountId,
          literal: "i105testliteral",
          network_prefix: 73,
          error_correction: "M",
          modules: 192,
          qr_version: 5,
          svg: "<svg />",
        },
        headers: { "content-type": "application/json" },
      });
    }
    assert.equal(requestUrl.search, "");
    return createResponse({
      status: 200,
      jsonData: {
        canonical_id: accountId,
        literal: "i105defaultliteral",
        network_prefix: 73,
        error_correction: "M",
        modules: 192,
        qr_version: 5,
        svg: "<svg />",
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const firstSnapshot = await client.getExplorerAccountQr(accountId);
  assert.deepEqual(firstSnapshot, {
    canonicalId: accountId,
    literal: "i105testliteral",
    networkPrefix: 73,
    errorCorrection: "M",
    modules: 192,
    qrVersion: 5,
    svg: "<svg />",
  });
  const defaultSnapshot = await client.getExplorerAccountQr(accountId);
  assert.equal(defaultSnapshot.literal, "i105defaultliteral");
  assert.equal(callCount, 2);
});

test("getExplorerAccountQr rejects non-object options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () => client.getExplorerAccountQr(FIXTURE_ALICE_ID, 42),
    /getExplorerAccountQr options must be an object/,
  );
});

test("getExplorerAccountQr rejects unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () => client.getExplorerAccountQr(FIXTURE_ALICE_ID, { legacyFormat: "i105", extra: true }),
    /getExplorerAccountQr options contains unsupported fields: legacyFormat, extra/,
  );
});

test("registerSnsName posts payload and normalizes response", async () => {
  const nameHash = "cd".repeat(32);
  const highestCommitment = "ab".repeat(32);
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    assert.equal(url, `${BASE_URL}/v1/sns/names`);
    assert.equal(init.method, "POST");
    const parsed = JSON.parse(init.body);
    assert.equal(parsed.selector.suffix_id, 1);
    assert.equal(parsed.selector.label, "makoto");
    assert.equal(parsed.owner, SAMPLE_ACCOUNT_ID);
    assert.equal(parsed.payment.asset_id, FIXTURE_ASSET_ID_A);
    return createResponse({
      status: 201,
      jsonData: {
        name_record: {
          selector: { version: 1, suffix_id: 1, label: "makoto" },
          name_hash: nameHash,
          owner: SAMPLE_ACCOUNT_ID,
          controllers: [
            { controller_type: "Account", account_address: "soraowner", payload: { note: "primary" } },
          ],
          status: { status: "Frozen", detail: { reason: "review", until_ms: 42 } },
          pricing_class: 2,
          registered_at_ms: 10,
          expires_at_ms: 20,
          grace_expires_at_ms: 30,
          redemption_expires_at_ms: 40,
          metadata: { note: "hi" },
          auction: {
            kind: "VickreyCommitReveal",
            opened_at_ms: 1,
            closes_at_ms: 2,
            floor_price: { asset_id: FIXTURE_ASSET_ID_A, amount: "120" },
            highest_commitment: highestCommitment,
            settlement_tx: { tx: "abc" },
          },
        },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const response = await client.registerSnsName({
    selector: { suffix_id: 1, label: "makoto" },
    owner: SAMPLE_ACCOUNT_ID,
    term_years: 1,
    controllers: [{ controller_type: "Account", account_address: "soraowner", payload: { note: "primary" } }],
    payment: {
      asset_id: FIXTURE_ASSET_ID_A,
      gross_amount: 120,
      settlement_tx: { tx: "abc" },
      payer: SAMPLE_ACCOUNT_ID,
      signature: "sig",
    },
    metadata: { note: "hi" },
  });
  assert.equal(captured.init.headers["Content-Type"], "application/json");
  assert.equal(response.nameRecord.nameHash, nameHash);
  assert.equal(response.nameRecord.status.status, "Frozen");
  assert.equal(response.nameRecord.status.reason, "review");
  assert.equal(response.nameRecord.auction?.highestCommitment, highestCommitment);
});

test("SNS mutation helpers reject unsupported option fields", async () => {
  const fetchImpl = async () => {
    throw new Error("fetch should not run for invalid options");
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const sampleRegisterPayload = {
    selector: { suffix_id: 1, label: "makoto" },
    owner: SAMPLE_ACCOUNT_ID,
    term_years: 1,
    controllers: [{ controller_type: "Account", account_address: SAMPLE_ACCOUNT_ID }],
    payment: {
      asset_id: FIXTURE_ASSET_ID_A,
      gross_amount: 120,
      settlement_tx: { tx: "abc" },
      payer: SAMPLE_ACCOUNT_ID,
      signature: "sig",
    },
  };
  const sampleRenewPayload = {
    term_years: 1,
    payment: {
      asset_id: FIXTURE_ASSET_ID_A,
      gross_amount: 120,
      settlement_tx: { tx: "abc" },
      payer: SAMPLE_ACCOUNT_ID,
      signature: "sig",
    },
  };
  const sampleTransferPayload = {
    new_owner: SAMPLE_ACCOUNT_ID,
    governance: {
      proposal_id: "proposal-1",
      council_vote_hash: "aa".repeat(32),
      dao_vote_hash: "bb".repeat(32),
      steward_ack: "ack",
      guardian_clearance: "guardian-ok",
    },
  };
  await assert.rejects(
    () => client.registerSnsName(sampleRegisterPayload, { extra: true }),
    /registerSnsName options contains unsupported fields: extra/,
  );
  await assert.rejects(
    () => client.renewSnsRegistration("makoto.sora", sampleRenewPayload, { note: "nope" }),
    /renewSnsRegistration options contains unsupported fields: note/,
  );
  await assert.rejects(
    () => client.transferSnsRegistration("makoto.sora", sampleTransferPayload, { retry: false }),
    /transferSnsRegistration options contains unsupported fields: retry/,
  );
});

test("getSnsPolicy fetches and normalizes suffix policy", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(url, `${BASE_URL}/v1/sns/policies/2`);
    assert.equal(init.method, "GET");
    return createResponse({
      status: 200,
      jsonData: {
        suffix_id: 2,
        suffix: "SORA",
        steward: SAMPLE_ACCOUNT_ID,
        status: "Active",
        min_term_years: 1,
        max_term_years: 5,
        grace_period_days: 30,
        redemption_period_days: 60,
        referral_cap_bps: 100,
        reserved_labels: [{ normalized_label: "gov", assigned_to: null, release_at_ms: null, note: "reserved" }],
        payment_asset_id: FIXTURE_ASSET_ID_A,
        pricing: [
          {
            tier_id: 1,
            label_regex: ".*",
            base_price: { asset_id: FIXTURE_ASSET_ID_A, amount: "100" },
            auction_kind: "DutchReopen",
            dutch_floor: { asset_id: FIXTURE_ASSET_ID_A, amount: "10" },
            min_duration_years: 1,
            max_duration_years: 5,
          },
        ],
        fee_split: { treasury_bps: 7000, steward_bps: 3000, referral_max_bps: 0, escrow_bps: 0 },
        fund_splitter_account: SAMPLE_ACCOUNT_ID,
        policy_version: 1,
        metadata: { version: "v1" },
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const policy = await client.getSnsPolicy(2);
  assert.equal(policy.suffixId, 2);
  assert.equal(policy.suffix, "sora");
  assert.equal(policy.pricing[0].auctionKind, "DutchReopen");
  assert.equal(policy.reservedLabels[0].normalizedLabel, "gov");
});

test("SNS read helpers reject unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run for option validation");
    },
  });
  await assert.rejects(
    () => client.getSnsPolicy(1, { extra: true }),
    /getSnsPolicy options contains unsupported fields: extra/,
  );
  await assert.rejects(
    () => client.getSnsRegistration("alice.sora", { retry: false }),
    /getSnsRegistration options contains unsupported fields: retry/,
  );
});

test("registerSnsName rejects invalid controller types", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () =>
      client.registerSnsName({
        selector: { suffix_id: 1, label: "bad" },
        owner: SAMPLE_ACCOUNT_ID,
        payment: {
          asset_id: FIXTURE_ASSET_ID_A,
          gross_amount: 1,
          settlement_tx: {},
          payer: SAMPLE_ACCOUNT_ID,
          signature: {},
        },
        controllers: [{ controller_type: "Unknown" }],
      }),
    /controller_type must be one of/,
  );
});

test("freezeSnsRegistration posts normalized payload and parses record", async () => {
  const controller = new AbortController();
  let captured;
  const nameHash = "ab".repeat(32);
  const responseBody = {
    selector: { version: 1, suffix_id: 2, label: " alice " },
    name_hash: `0x${nameHash}`,
    owner: SAMPLE_ACCOUNT_FORMS.i105,
    controllers: [{ controller_type: "Account", account_address: SAMPLE_ACCOUNT_FORMS.i105 }],
    status: { status: "Frozen", reason: "timeout", until_ms: 7000 },
    pricing_class: 2,
    registered_at_ms: 10,
    expires_at_ms: 20,
    grace_expires_at_ms: 30,
    redemption_expires_at_ms: 40,
    metadata: { note: "test" },
  };
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url, init) => {
      captured = { url, init };
      return createResponse({
        status: 200,
        jsonData: responseBody,
        headers: { "content-type": "application/json" },
      });
    },
  });
  const result = await client.freezeSnsRegistration(
    "  alice.sora  ",
    { reason: "  abuse report ", until_ms: 5000, guardian_ticket: { token: "grant" } },
    { signal: controller.signal },
  );
  assert.equal(captured.url, `${BASE_URL}/v1/sns/names/domain/alice/freeze`);
  assert.equal(captured.init.method, "POST");
  assert.deepEqual(JSON.parse(String(captured.init.body)), {
    reason: "abuse report",
    until_ms: 5000,
    guardian_ticket: { token: "grant" },
  });
  assert.ok(captured.init.signal instanceof AbortSignal);
  assert.equal(result.selector.label, "alice");
  assert.equal(result.selector.suffix_id, 2);
  assert.equal(result.nameHash, nameHash);
  assert.equal(result.owner, SAMPLE_ACCOUNT_ID);
  assert.deepEqual(result.status, { status: "Frozen", reason: "timeout", untilMs: 7000 });
  assert.equal(result.pricingClass, 2);
  assert.equal(result.registeredAtMs, 10);
  assert.equal(result.controllers.length, 1);
  assert.equal(result.controllers[0]?.controller_type, "Account");
});

test("unfreezeSnsRegistration posts governance hook payload", async () => {
  let captured;
  const responseBody = {
    selector: { version: 1, suffix_id: 2, label: "alice" },
    name_hash: "ff".repeat(32),
    owner: SAMPLE_ACCOUNT_FORMS.i105,
    controllers: [{ controller_type: "Account", account_address: SAMPLE_ACCOUNT_FORMS.i105 }],
    status: "Active",
    pricing_class: 3,
    registered_at_ms: 100,
    expires_at_ms: 200,
    grace_expires_at_ms: 300,
    redemption_expires_at_ms: 400,
  };
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async (url, init) => {
      captured = { url, init };
      return createResponse({
        status: 200,
        jsonData: responseBody,
        headers: { "content-type": "application/json" },
      });
    },
  });
  const result = await client.unfreezeSnsRegistration(" alice.sora ", {
    proposal_id: " prop-123 ",
    council_vote_hash: " council-hash ",
    dao_vote_hash: " dao-hash ",
    steward_ack: " ack ",
    guardian_clearance: " clearance ",
  });
  assert.equal(captured.url, `${BASE_URL}/v1/sns/names/domain/alice/freeze`);
  assert.equal(captured.init.method, "DELETE");
  assert.deepEqual(JSON.parse(String(captured.init.body)), {
    proposal_id: "prop-123",
    council_vote_hash: "council-hash",
    dao_vote_hash: "dao-hash",
    steward_ack: "ack",
    guardian_clearance: "clearance",
  });
  assert.equal(result.selector.label, "alice");
  assert.equal(result.owner, SAMPLE_ACCOUNT_ID);
  assert.deepEqual(result.status, { status: "Active" });
  assert.equal(result.pricingClass, 3);
});

test("freeze/unfreeze SNS governance helpers enforce validation", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run for validation failures");
    },
  });
  await assert.rejects(
    () =>
      client.freezeSnsRegistration("alice.sora", {
        reason: "r",
        until_ms: 1,
      }),
    /freezeSnsRegistration\.guardian_ticket must be provided/,
  );
  await assert.rejects(
    () =>
      client.freezeSnsRegistration(
        "alice.sora",
        { reason: "ok", until_ms: 1, guardian_ticket: {} },
        // @ts-expect-error runtime validation
        { extra: true },
      ),
    /freezeSnsRegistration options contains unsupported fields: extra/,
  );
  await assert.rejects(
    () =>
      client.unfreezeSnsRegistration(
        "alice.sora",
        {
          proposal_id: "p",
          council_vote_hash: "c",
          dao_vote_hash: "d",
          steward_ack: "s",
        },
        // @ts-expect-error runtime validation
        { retry: false },
      ),
    /unfreezeSnsRegistration options contains unsupported fields: retry/,
  );
});

test("createSnsGovernanceCase rejects because Torii removed the endpoint", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run when the endpoint is removed");
    },
  });
  await assert.rejects(
    () => client.createSnsGovernanceCase({ reason: "abuse-report", selector: "alice" }),
    /Torii removed \/v1\/sns\/governance\/cases/,
  );
});

test("createSnsGovernanceCase rejects non-object payload", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run for validation failures");
    },
  });
  await assert.rejects(
    // @ts-expect-error runtime validation
    () => client.createSnsGovernanceCase("not-an-object"),
    /createSnsGovernanceCase payload must be an object/,
  );
});

test("createSnsGovernanceCase rejects unsupported option keys", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run for validation failures");
    },
  });
  const controller = new AbortController();
  await assert.rejects(
    () =>
      client.createSnsGovernanceCase(
        { reason: "abuse-report", selector: "alice" },
        // @ts-expect-error runtime validation
        { signal: controller.signal, retry: false },
      ),
    /createSnsGovernanceCase options contains unsupported fields: retry/,
  );
});

test("createSnsGovernanceCase still validates payload before reporting endpoint removal", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run when the endpoint is removed");
    },
  });
  await assert.rejects(
    () =>
      client.createSnsGovernanceCase({
        selector: { suffixId: 7, label: "alice", globalForm: "alice.sora" },
        disputeType: "ownership",
        priority: "high",
        reason: "abuse-report",
      }),
    /Torii removed \/v1\/sns\/governance\/cases/,
  );
});

test("createSnsGovernanceCase rejects invalid dispute type before calling Torii", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run for validation failures");
    },
  });
  await assert.rejects(
    () =>
      client.createSnsGovernanceCase({
        selector: "alice",
        disputeType: "not-supported",
      }),
    /createSnsGovernanceCase payload\.dispute_type must be one of/,
  );
});

test("exportSnsGovernanceCases rejects because Torii removed the endpoint", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run when the endpoint is removed");
    },
  });
  await assert.rejects(
    () =>
      client.exportSnsGovernanceCases({
        since: "2026-01-01T00:00:00Z",
        status: "open",
        limit: 10,
      }),
    /Torii removed \/v1\/sns\/governance\/cases/,
  );
});

test("exportSnsGovernanceCases enforces option validation", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not be invoked for invalid options");
    },
  });
  await assert.rejects(
    // @ts-expect-error runtime validation
    () => client.exportSnsGovernanceCases("oops"),
    /exportSnsGovernanceCases options must be an object/,
  );
  await assert.rejects(
    () => client.exportSnsGovernanceCases({ since: 123 }),
    /exportSnsGovernanceCases\.since must be a string/,
  );
  await assert.rejects(
    () => client.exportSnsGovernanceCases({ status: "   " }),
    /exportSnsGovernanceCases\.status must not be empty/,
  );
  await assert.rejects(
    () => client.exportSnsGovernanceCases({ limit: -1 }),
    /exportSnsGovernanceCases\.limit must be a non-negative integer/,
  );
  await assert.rejects(
    () => client.exportSnsGovernanceCases({ since: "2026-01-01", unexpected: true }),
    /exportSnsGovernanceCases options contains unsupported fields: unexpected/,
  );
});

test("iterateSnsGovernanceCases rejects because Torii removed the endpoint", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run when the endpoint is removed");
    },
  });
  await assert.rejects(
    async () => {
      for await (const _ of client.iterateSnsGovernanceCases({
        since: "2026-01-01T00:00:00Z",
        limit: 1,
      })) {
        // no-op
      }
    },
    /Torii removed \/v1\/sns\/governance\/cases/,
  );
});

test("listTelemetryPeersInfo normalizes peer telemetry metadata", async () => {
  let captured;
  const payload = [
    {
      url: "https://peer-1.example",
      connected: true,
      telemetry_unsupported: false,
      config: {
        public_key: SAMPLE_ACCOUNT_SIGNATORY,
        queue_capacity: 16,
        network_block_gossip_size: 32,
        network_block_gossip_period: { ms: 150 },
        network_tx_gossip_size: 8,
        network_tx_gossip_period: { ms: 75 },
      },
      location: { lat: 35.681, lon: 139.767, country: "JP", city: "Tokyo" },
      connected_peers: ["peer-A", "peer-B"],
    },
  ];
  const fetchImpl = async (url, init) => {
    captured = { url, init };
    return createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const peers = await client.listTelemetryPeersInfo();
  assert.equal(captured.url, `${BASE_URL}/v1/telemetry/peers-info`);
  assert.equal(captured.init.method, "GET");
  assert.equal(captured.init.headers.Accept, "application/json");
  assert.deepEqual(peers, [
    {
      url: "https://peer-1.example",
      connected: true,
      telemetryUnsupported: false,
      config: {
        publicKey: SAMPLE_ACCOUNT_SIGNATORY,
        queueCapacity: 16,
        networkBlockGossipSize: 32,
        networkBlockGossipPeriodMs: 150,
        networkTxGossipSize: 8,
        networkTxGossipPeriodMs: 75,
      },
      location: { lat: 35.681, lon: 139.767, country: "JP", city: "Tokyo" },
      connectedPeers: ["peer-A", "peer-B"],
    },
  ]);
});

test("listTelemetryPeersInfo rejects non-object options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not be invoked");
    },
  });
  await assert.rejects(
    () => client.listTelemetryPeersInfo(Symbol("options")),
    /listTelemetryPeersInfo options must be an object/,
  );
});

test("listTelemetryPeersInfo rejects unsupported option fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not be invoked");
    },
  });
  await assert.rejects(
    () => client.listTelemetryPeersInfo({ extra: "nope" }),
    /listTelemetryPeersInfo options contains unsupported fields: extra/,
  );
});

test("listTelemetryPeersInfo rejects malformed payloads", async () => {
  const fetchImpl = async () =>
    createResponse({
      status: 200,
      jsonData: { not: "an array" },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listTelemetryPeersInfo(),
    /telemetry peers response must be an array/,
  );
});

test("methods surface HTTP errors with body", async () => {
  const fetchImpl = async () =>
    createResponse({ status: 500, textBody: "boom", jsonData: { error: "boom" } });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listAttachments(),
    (error) => {
      assert(error instanceof ToriiHttpError);
      assert.equal(error.status, 500);
      assert.equal(error.errorMessage, "boom");
      assert.match(
        error.message,
        /Torii responded with HTTP 500 \(expected 200\): boom/,
      );
      return true;
    },
  );
});

test("http errors expose structured fields", async () => {
  const payload = {
    code: "ERR_ACCOUNT_LITERAL_FORMAT",
    message: "invalid account literal",
  };
  const fetchImpl = async () =>
    createResponse({
      status: 400,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.listAccounts(),
    (error) => {
      assert(error instanceof ToriiHttpError);
      assert.equal(error.status, 400);
      assert.deepEqual(error.expected, [200]);
      assert.equal(error.code, payload.code);
      assert.equal(error.errorMessage, payload.message);
      assert.equal(error.bodyJson, payload);
      return true;
    },
  );
});

test("http errors surface reject header codes", async () => {
  const fetchImpl = async (url) => {
    if (url === `${BASE_URL}/v1/node/capabilities`) {
      return createResponse({
        status: 200,
        jsonData: {
          abi_version: 1,
          data_model_version: 1,
          crypto: {
            sm: {
              enabled: false,
              default_hash: "sha2_256",
              allowed_signing: ["ed25519"],
              sm2_distid_default: "",
              openssl_preview: false,
              acceleration: {
                scalar: true,
                neon_sm3: false,
                neon_sm4: false,
                policy: "scalar-only",
              },
            },
            curves: {
              registry_version: 1,
              allowed_curve_ids: [1],
            },
          },
        },
        headers: { "content-type": "application/json" },
      });
    }
    return createResponse({
      status: 400,
      jsonData: { message: "failed to accept transaction" },
      headers: {
        "content-type": "application/json",
        "x-iroha-reject-code": "PRTRY:TX_SIGNATURE_MISSING",
      },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.submitTransaction(new Uint8Array([0x01, 0x02])),
    (error) => {
      assert(error instanceof ToriiHttpError);
      assert.equal(error.status, 400);
      assert.equal(error.rejectCode, "PRTRY:TX_SIGNATURE_MISSING");
      assert.equal(error.code, "PRTRY:TX_SIGNATURE_MISSING");
      return true;
    },
  );
});

function requireSorafsNative(t) {
  const native = getNativeBinding();
  if (
    !native ||
    typeof native.sorafsAliasPolicyDefaults !== "function" ||
    typeof native.sorafsAliasProofFixture !== "function" ||
    typeof native.sorafsEvaluateAliasProof !== "function"
  ) {
    if (t && typeof t.skip === "function") {
      t.skip(nativeSkipMessage);
      return null;
    }
    throw new Error(
      "SoraFS native helpers unavailable. Run `npm run build:native` before executing tests.",
    );
  }
  return native;
}

function createResponse({ status, jsonData = {}, arrayData, textBody, headers }) {
  return {
    status,
    json: async () => jsonData,
    arrayBuffer: async () => {
      if (arrayData instanceof ArrayBuffer) {
        return arrayData;
      }
      if (ArrayBuffer.isView(arrayData)) {
        return arrayData.buffer.slice(arrayData.byteOffset, arrayData.byteOffset + arrayData.byteLength);
      }
      return new TextEncoder().encode(textBody ?? JSON.stringify(jsonData ?? {})).buffer;
    },
    text: async () => (typeof textBody === "string" ? textBody : JSON.stringify(jsonData ?? {})),
    headers: {
      get(name) {
        if (!headers) {
          return null;
        }
        const normalized = name.toLowerCase();
        for (const [key, value] of Object.entries(headers)) {
          if (key.toLowerCase() === normalized) {
            return value;
          }
        }
        return null;
      },
    },
  };
}

test("ToriiClient._normalizeUnsignedInteger enforces integer inputs", () => {
  assert.equal(ToriiClient._normalizeUnsignedInteger("42", "value"), 42);
  assert.equal(ToriiClient._normalizeUnsignedInteger(0, "value", { allowZero: true }), 0);
  assert.equal(ToriiClient._normalizeUnsignedInteger(42n, "value"), 42);
  assert.throws(
    () => ToriiClient._normalizeUnsignedInteger(1.5, "value"),
    /value must be a positive integer/,
  );
  assert.throws(
    () => ToriiClient._normalizeUnsignedInteger("1.5", "value"),
    /value must be a positive integer/,
  );
  assert.throws(
    () => ToriiClient._normalizeUnsignedInteger(Number.MAX_SAFE_INTEGER + 1, "value"),
    /value must be at most/,
  );
});

test("ToriiClient._normalizeOffset rejects fractional offsets", () => {
  assert.throws(
    () => ToriiClient._normalizeOffset(1.25),
    /offset must be a non-negative integer/,
  );
});

async function fileExists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

function createSseResponse(chunks) {
  const body = {
    async *[Symbol.asyncIterator]() {
      const encoder = new TextEncoder();
      for (const chunk of chunks) {
        yield encoder.encode(chunk);
      }
    },
  };
  return {
    status: 200,
    headers: {
      get(name) {
        return name.toLowerCase() === "content-type" ? "text/event-stream" : null;
      },
    },
    body,
  };
}

async function withEnv(overrides, fn) {
  const original = {};
  for (const [key, value] of Object.entries(overrides)) {
    original[key] = process.env[key];
    if (value === null || value === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = value;
    }
  }
  try {
    await fn();
  } finally {
    for (const [key, value] of Object.entries(original)) {
      if (value === undefined) {
        delete process.env[key];
      } else {
        process.env[key] = value;
      }
    }
  }
}

import assert from "node:assert/strict";
import test from "node:test";

import { canonicalRequestMessage } from "../src/canonicalRequest.js";
import { signEd25519, verifyEd25519 } from "../src/crypto.js";
import { NetworkId, networkIdBytes } from "../src/networkId.js";
import { buildOperatorRequestHeaders } from "../src/operatorRequest.js";
import {
  OperatorSigningContext,
  ToriiClient,
} from "../src/toriiClient.js";

const BASE_URL = "https://torii.example";
const NETWORK_ID = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);
const PRIVATE_KEY = Buffer.alloc(32, 0x0b);
const PUBLIC_KEY = Buffer.from(
  "66BE7E332C7A453332BD9D0A7F7DB055F5C5EF1A06ADA66D98B39FB6810C473A",
  "hex",
);
const PUBLIC_KEY_MULTIHASH = `ed0120${PUBLIC_KEY.toString("hex").toUpperCase()}`;

function signingContext(networkId = NETWORK_ID) {
  return new OperatorSigningContext(networkId, {
    publicKey: PUBLIC_KEY_MULTIHASH,
    sign: (message) => signEd25519(message, PRIVATE_KEY),
  });
}

function jsonResponse(status, payload) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function header(headers, name) {
  const entry = Object.entries(headers).find(
    ([candidate]) => candidate.toLowerCase() === name.toLowerCase(),
  );
  return entry?.[1];
}

test("operator header builders enforce the prepared wire-query byte cap", async () => {
  const rawQuery = `x=${"é".repeat(32_767)}`;
  assert.equal(Buffer.byteLength(rawQuery, "utf8"), 65_536);
  assert.doesNotThrow(() =>
    canonicalRequestMessage({ method: "GET", path: "/v1/test", query: rawQuery }),
  );

  await assert.rejects(
    buildOperatorRequestHeaders({
      signingContext: signingContext(),
      method: "GET",
      path: "/v1/test",
      query: rawQuery,
      timestampMs: 1,
      nonce: "prepared-operator-query-cap",
    }),
    /exceeds 65536 raw UTF-8 bytes/u,
  );
});

test("ISO submission signs the exact network, method, query, and body", async () => {
  let captured;
  const xml = Buffer.from("<Document><MsgId>signed-1</MsgId></Document>");
  const client = new ToriiClient(BASE_URL, {
    operatorSigningContext: signingContext(),
    fetchImpl: async (url, init) => {
      captured = { url, init };
      return jsonResponse(202, { message_id: "signed-1", status: "Accepted" });
    },
  });

  await client.submitIsoPacs008(xml, { profile: "swift-cbpr-plus" });

  assert.equal(
    captured.url,
    `${BASE_URL}/v1/iso20022/pacs008?profile=swift-cbpr-plus`,
  );
  assert.equal(captured.init.redirect, "error");
  assert.equal(header(captured.init.headers, "x-iroha-iso-profile"), undefined);
  assert.equal(
    header(captured.init.headers, "x-iroha-operator-public-key"),
    PUBLIC_KEY_MULTIHASH,
  );
  const timestamp = header(captured.init.headers, "x-iroha-operator-timestamp-ms");
  const nonce = header(captured.init.headers, "x-iroha-operator-nonce");
  const signature = Buffer.from(
    header(captured.init.headers, "x-iroha-operator-signature"),
    "base64",
  );
  const request = canonicalRequestMessage({
    method: "POST",
    path: "/v1/iso20022/pacs008",
    query: "profile=swift-cbpr-plus",
    body: xml,
  });
  const message = Buffer.concat([
    Buffer.from("iroha.operator.http-request.network.v1\0"),
    Buffer.from(networkIdBytes(NETWORK_ID)),
    request,
    Buffer.from(`\n${timestamp}\n${nonce}`),
  ]);
  assert.equal(verifyEd25519(message, signature, PUBLIC_KEY), true);
  const foreignMessage = Buffer.concat([
    Buffer.from("iroha.operator.http-request.network.v1\0"),
    Buffer.from(networkIdBytes(NETWORK_ID)),
    canonicalRequestMessage({
      method: "POST",
      path: "/v1/iso20022/pacs008",
      query: "profile=foreign-profile",
      body: xml,
    }),
    Buffer.from(`\n${timestamp}\n${nonce}`),
  ]);
  assert.equal(
    verifyEd25519(foreignMessage, signature, PUBLIC_KEY),
    false,
  );
});

test("ISO operator requests are mandatory, one-shot, and reject retired auth", async () => {
  let calls = 0;
  const unsigned = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      calls += 1;
      return jsonResponse(202, {});
    },
  });
  await assert.rejects(
    () => unsigned.submitIsoPacs008("<xml/>"),
    /requires an immutable OperatorSigningContext/u,
  );
  assert.equal(calls, 0);

  const retrying = new ToriiClient(BASE_URL, {
    operatorSigningContext: signingContext(),
    maxRetries: 5,
    retryMethods: ["POST"],
    retryStatuses: [503],
    fetchImpl: async (_url, init) => {
      calls += 1;
      assert.equal(init.redirect, "error");
      return jsonResponse(503, { error: "unavailable" });
    },
  });
  await assert.rejects(() => retrying.submitIsoPacs009("<xml/>"));
  assert.equal(calls, 1);

  for (const options of [
    { apiToken: "retired-token" },
    { authToken: "retired-bearer" },
    { defaultHeaders: { "X-Iroha-Account": "retired-app-auth" } },
    { defaultHeaders: { "X-Iroha-Iso-Profile": "legacy-profile" } },
    { defaultHeaders: { "X-Iroha-Operator-Nonce": "precomputed" } },
  ]) {
    const client = new ToriiClient(BASE_URL, {
      ...options,
      operatorSigningContext: signingContext(),
      fetchImpl: async () => {
        throw new Error("retired auth must fail before dispatch");
      },
    });
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(
      () => client.getIsoMessageStatus("signed-1"),
      /requires generated operator signing/u,
    );
  }
});

test("ISO profiles require exact canonical catalog identifiers", async () => {
  const client = new ToriiClient(BASE_URL, {
    operatorSigningContext: signingContext(),
    fetchImpl: async () => {
      throw new Error("invalid profiles must fail before dispatch");
    },
  });
  for (const profile of [" Swift-CBPR-Plus", "swift_cbpr_plus", "swift-"]) {
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(
      () => client.submitIsoPacs008("<xml/>", { profile }),
      /canonical lowercase profile id/u,
    );
  }
});

test("each ISO status poll receives a fresh operator nonce", async () => {
  const nonces = [];
  let attempts = 0;
  const client = new ToriiClient(BASE_URL, {
    operatorSigningContext: signingContext(),
    fetchImpl: async (_url, init) => {
      nonces.push(header(init.headers, "x-iroha-operator-nonce"));
      attempts += 1;
      return jsonResponse(200, {
        message_id: "poll-1",
        status: attempts === 1 ? "Pending" : "Committed",
        transaction_hash: attempts === 1 ? null : "tx-1",
        pacs002_code: attempts === 1 ? "PDNG" : "ACSC",
        detail: null,
        updated_at_ms: attempts,
      });
    },
  });

  await client.waitForIsoMessageStatus("poll-1", {
    maxAttempts: 2,
    pollIntervalMs: 0,
  });
  assert.equal(nonces.length, 2);
  assert.notEqual(nonces[0], nonces[1]);
});

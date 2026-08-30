import assert from "node:assert/strict";
import { test } from "node:test";

import { ToriiClient } from "../src/toriiClient.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";
import { ToriiClient as DistToriiClient } from "../dist/toriiClient.js";
import { ToriiBrowserClient as DistToriiBrowserClient } from "../dist/toriiBrowserClient.js";

const BASE_URL = "https://torii.example";
const EXACT_QUERY = "status=Active&ids_only=true&limit=1000&order=asc";
const MAX_RESPONSE_BYTES = 512 * 1024;
const VALID_BODY = JSON.stringify([
  { backend: "halo2/ipa", name: "a:b" },
  { backend: "halo2/ipa", name: "a".repeat(256) },
  {
    backend:
      "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
    name: "kagemusha_topup_v3",
  },
  { backend: "halo2/ipa", name: "same_name" },
  { backend: "stark/fri", name: "same_name" },
  { backend: "stark/fri", name: "stark_transfer_v1" },
]);

function rawResponse(body, { contentType = "application/json", headers = {} } = {}) {
  return new Response(body, {
    status: 200,
    headers: {
      "Content-Type": contentType,
      ...headers,
    },
  });
}

function headerValue(headers, name) {
  return new Headers(headers).get(name);
}

const SURFACES = Object.freeze([
  Object.freeze({ label: "Node source", Client: ToriiClient }),
  Object.freeze({ label: "Node dist", Client: DistToriiClient }),
  Object.freeze({ label: "browser source", Client: ToriiBrowserClient }),
  Object.freeze({ label: "browser dist", Client: DistToriiBrowserClient }),
]);

test("active verifying-key discovery is exact, ordered, public, and identical across package surfaces", async () => {
  for (const { label, Client } of SURFACES) {
    let request = null;
    const client = new Client(BASE_URL, {
      authToken: "must-not-leak",
      apiToken: "must-not-leak",
      defaultHeaders: {
        accept: "text/json",
        Authorization: "Bearer must-not-leak",
        Cookie: "session=must-not-leak",
        "Proxy-Authorization": "must-not-leak",
        "x-ACCOUNT-id": "must-not-leak",
        "X-Dataspace-Id": "must-not-leak",
        "X-Iroha-Account": "must-not-leak",
        "X-Iroha-Nonce": "must-not-leak",
        "x-IROHA-onboarding-TOKEN": "must-not-leak",
        "Accept-Encoding": "gzip",
        "x-Iroha-Operator-Nonce": "must-not-leak",
        "X-IROHA-OPERATOR-PUBLIC-KEY": "must-not-leak",
        "X-Iroha-Operator-Signature": "must-not-leak",
        "x-iroha-operator-timestamp-ms": "must-not-leak",
        "x-iroha-operator-future-credential": "must-not-leak",
        "X-Iroha-Signature": "must-not-leak",
        "X-Iroha-Timestamp-Ms": "must-not-leak",
        "X-Iroha-Witness": "must-not-leak",
        "X-API-Token": "must-not-leak",
        "X-Trace-Id": "trace-must-remain",
      },
      fetchImpl: async (url, init) => {
        request = { url, init };
        return rawResponse(VALID_BODY, {
          headers: { "Content-Length": String(Buffer.byteLength(VALID_BODY)) },
        });
      },
    });

    const ids = await client.listActiveVerifyingKeyIds();
    assert.equal(Object.isFrozen(ids), true, `${label} freezes the projection`);
    assert.equal(Object.isFrozen(ids[0]), true, `${label} freezes each identifier`);
    assert.deepEqual(
      ids.map(({ backend, name }) => ({ backend, name })),
      JSON.parse(VALID_BODY),
      label,
    );
    assert.equal(new URL(request.url).pathname, "/v1/zk/vk", label);
    assert.equal(new URL(request.url).search.slice(1), EXACT_QUERY, label);
    const acceptHeaders = Object.entries(request.init.headers).filter(
      ([name]) => name.toLowerCase() === "accept",
    );
    assert.equal(acceptHeaders.length, 1, `${label} emitted duplicate Accept headers`);
    assert.equal(acceptHeaders[0][1], "application/json", label);
    for (const credential of [
      "authorization",
      "cookie",
      "proxy-authorization",
      "x-account-id",
      "x-api-token",
      "x-dataspace-id",
      "x-iroha-account",
      "x-iroha-nonce",
      "x-iroha-onboarding-token",
      "x-iroha-operator-nonce",
      "x-iroha-operator-public-key",
      "x-iroha-operator-signature",
      "x-iroha-operator-timestamp-ms",
      "x-iroha-operator-future-credential",
      "x-iroha-signature",
      "x-iroha-timestamp-ms",
      "x-iroha-witness",
    ]) {
      assert.equal(
        headerValue(request.init.headers, credential),
        null,
        `${label} leaked ${credential}`,
      );
    }
    assert.equal(
      headerValue(request.init.headers, "x-trace-id"),
      "trace-must-remain",
      `${label} removed a harmless default header`,
    );
    assert.equal(request.init.credentials, "omit", label);
    assert.equal(request.init.redirect, "error", label);
    assert.equal(
      headerValue(request.init.headers, "accept-encoding"),
      label.startsWith("Node") ? "identity" : null,
      label,
    );
  }
});

test("active verifying-key discovery fails closed on hostile projections", async () => {
  const tooMany = JSON.stringify(
    Array.from({ length: 1_001 }, (_, index) => ({
      backend: "halo2/ipa",
      name: `vk_${String(index).padStart(4, "0")}`,
    })),
  );
  const hostileBodies = [
    ["object envelope", '{"items":[{"backend":"halo2/ipa","name":"vk"}]}'],
    ["unknown field", '[{"backend":"halo2/ipa","name":"vk","status":"Active"}]'],
    ["missing field", '[{"backend":"halo2/ipa"}]'],
    ["non-string field", '[{"backend":"halo2/ipa","name":7}]'],
    ["unknown backend", '[{"backend":"unsupported","name":"vk"}]'],
    ["uppercase backend", '[{"backend":"Halo2/ipa","name":"vk"}]'],
    ["Unicode backend", '[{"backend":"halo2/ipä","name":"vk"}]'],
    ["backend double slash", '[{"backend":"halo2//ipa","name":"vk"}]'],
    ["backend dangling separator", '[{"backend":"halo2/ipa/","name":"vk"}]'],
    ["padded name", '[{"backend":"halo2/ipa","name":" vk"}]'],
    ["uppercase name", '[{"backend":"halo2/ipa","name":"Vk"}]'],
    ["Unicode name", '[{"backend":"halo2/ipa","name":"vé"}]'],
    ["empty name", '[{"backend":"halo2/ipa","name":""}]'],
    ["leading separator", '[{"backend":"halo2/ipa","name":"/vk"}]'],
    ["dangling separator", '[{"backend":"halo2/ipa","name":"vk-"}]'],
    ["double dot", '[{"backend":"halo2/ipa","name":"vk..1"}]'],
    ["double slash", '[{"backend":"halo2/ipa","name":"vk//1"}]'],
    ["triple colon", '[{"backend":"halo2/ipa","name":"vk:::1"}]'],
    ["slash colon", '[{"backend":"halo2/ipa","name":"vk/:1"}]'],
    ["colon slash", '[{"backend":"halo2/ipa","name":"vk:/1"}]'],
    ["slash dot", '[{"backend":"halo2/ipa","name":"vk/.1"}]'],
    ["dot slash", '[{"backend":"halo2/ipa","name":"vk./1"}]'],
    ["colon dot", '[{"backend":"halo2/ipa","name":"vk:.1"}]'],
    ["dot colon", '[{"backend":"halo2/ipa","name":"vk.:1"}]'],
    ["overlong name", `[{"backend":"halo2/ipa","name":"${"a".repeat(257)}"}]`],
    ["duplicate JSON key", '[{"backend":"halo2/ipa","backend":"stark/fri","name":"vk"}]'],
    ["duplicate id", '[{"backend":"halo2/ipa","name":"vk"},{"backend":"halo2/ipa","name":"vk"}]'],
    ["name order drift", '[{"backend":"stark/fri","name":"z"},{"backend":"halo2/ipa","name":"a"}]'],
    ["equal-name backend order drift", '[{"backend":"stark/fri","name":"same"},{"backend":"halo2/ipa","name":"same"}]'],
    ["too many ids", tooMany],
  ];

  for (const { label, Client } of SURFACES) {
    for (const [caseLabel, body] of hostileBodies) {
      const client = new Client(BASE_URL, {
        fetchImpl: async () => rawResponse(body),
      });
      await assert.rejects(
        () => client.listActiveVerifyingKeyIds(),
        undefined,
        `${label}: ${caseLabel}`,
      );
    }
  }
});

test("active verifying-key discovery accepts exactly 1000 ordered identifiers", async () => {
  const body = JSON.stringify(
    Array.from({ length: 1_000 }, (_, index) => ({
      backend: "halo2/ipa",
      name: `vk_${String(index).padStart(4, "0")}`,
    })),
  );
  for (const { label, Client } of SURFACES) {
    const ids = await new Client(BASE_URL, {
      fetchImpl: async () => rawResponse(body),
    }).listActiveVerifyingKeyIds();
    assert.equal(ids.length, 1_000, label);
  }
});

test("active verifying-key discovery requires exact JSON media type and bounded UTF-8 bodies", async () => {
  const boundaryJson = '[{"backend":"halo2/ipa","name":"boundary"}]';
  const boundaryBody = `${boundaryJson}${" ".repeat(
    MAX_RESPONSE_BYTES - Buffer.byteLength(boundaryJson),
  )}`;
  assert.equal(Buffer.byteLength(boundaryBody), MAX_RESPONSE_BYTES);

  for (const { label, Client } of SURFACES) {
    for (const contentType of [
      "application/json; charset=utf-8",
      "Application/JSON",
      "text/json",
    ]) {
      const client = new Client(BASE_URL, {
        fetchImpl: async () => rawResponse(VALID_BODY, { contentType }),
      });
      await assert.rejects(
        () => client.listActiveVerifyingKeyIds(),
        /Content-Type must be exactly application\/json/u,
        `${label}: ${contentType}`,
      );
    }

    const boundaryIds = await new Client(BASE_URL, {
      fetchImpl: async () => rawResponse(boundaryBody, {
        headers: {
          "Content-Encoding": "identity",
          "Content-Length": String(MAX_RESPONSE_BYTES),
        },
      }),
    }).listActiveVerifyingKeyIds();
    assert.equal(boundaryIds.length, 1, `${label}: exact-size boundary`);

    for (const contentEncoding of ["gzip", "br", "identity, gzip"]) {
      await assert.rejects(
        () => new Client(BASE_URL, {
          fetchImpl: async () => rawResponse(VALID_BODY, {
            headers: { "Content-Encoding": contentEncoding },
          }),
        }).listActiveVerifyingKeyIds(),
        /Content-Encoding must be identity/u,
        `${label}: ${contentEncoding}`,
      );
    }

    for (const contentLength of ["01", "+1", "1, 1"]) {
      await assert.rejects(
        () => new Client(BASE_URL, {
          fetchImpl: async () => rawResponse(VALID_BODY, {
            headers: { "Content-Length": contentLength },
          }),
        }).listActiveVerifyingKeyIds(),
        /Content-Length/u,
        `${label}: malformed Content-Length ${contentLength}`,
      );
    }

    await assert.rejects(
      () => new Client(BASE_URL, {
        fetchImpl: async () => rawResponse(VALID_BODY, {
          headers: { "Content-Length": String(Buffer.byteLength(VALID_BODY) + 1) },
        }),
      }).listActiveVerifyingKeyIds(),
      /Content-Length does not match/u,
      `${label}: mismatched Content-Length`,
    );

    const oversized = new Uint8Array(MAX_RESPONSE_BYTES + 1).fill(0x20);
    await assert.rejects(
      () => new Client(BASE_URL, {
        fetchImpl: async () => rawResponse(oversized),
      }).listActiveVerifyingKeyIds(),
      /524288-byte response limit|512 KiB|response limit/u,
      `${label}: oversized body`,
    );

    await assert.rejects(
      () => new Client(BASE_URL, {
        fetchImpl: async () => rawResponse(new Uint8Array([0x5b, 0x22, 0xff, 0x22, 0x5d])),
      }).listActiveVerifyingKeyIds(),
      /valid UTF-8/u,
      `${label}: invalid UTF-8`,
    );
  }
});

test("active verifying-key discovery rejects URL user-info before fetch", async () => {
  for (const { label, Client } of SURFACES) {
    let requests = 0;
    assert.throws(
      () => new Client("https://ambient:must-not-leak@torii.example", {
        fetchImpl: async () => {
          requests += 1;
          return rawResponse(VALID_BODY);
        },
      }),
      /userinfo/u,
      label,
    );
    assert.equal(requests, 0, label);
  }
});

test("active verifying-key discovery rejects caller-controlled query or auth options before fetch", async () => {
  for (const { label, Client } of SURFACES) {
    let requests = 0;
    const client = new Client(BASE_URL, {
      fetchImpl: async () => {
        requests += 1;
        return rawResponse(VALID_BODY);
      },
    });
    await assert.rejects(
      () => client.listActiveVerifyingKeyIds({ status: "Withdrawn" }),
      /unsupported/u,
      label,
    );
    assert.equal(requests, 0, label);
  }
});

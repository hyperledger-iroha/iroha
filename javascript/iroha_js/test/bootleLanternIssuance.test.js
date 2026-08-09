import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1,
  BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1,
  BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1,
  BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1,
  BOOTLE_LANTERN_ISSUANCE_ISSUE_PATH_V1,
  BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1,
  BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1,
  BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1,
  BootleLanternIssuanceClientErrorV1,
  BootleLanternIssuanceClientV1,
  BootleLanternIssuanceCredentialV1,
} from "../src/bootleLanternIssuance.js";

const clientContractFixture = JSON.parse(
  readFileSync(
    new URL(
      "../../../fixtures/privacy/bootle_lantern_issuance_client_v1.json",
      import.meta.url,
    ),
    "utf8",
  ),
);

function patterned(length) {
  const body = Uint8Array.from({ length }, (_, index) => index & 0xff);
  if (length === BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1) {
    body.set(new TextEncoder().encode("ILA1"), 0);
  } else if (length === BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1) {
    body.set(new TextEncoder().encode("ILA1"), 0);
    body.set(new TextEncoder().encode("ILQ1"), BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1);
  } else if (length === BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1) {
    body.set(new TextEncoder().encode("ILR1"), 0);
  }
  return body;
}

function headerBag(entries) {
  return {
    raw() {
      return entries;
    },
  };
}

function response(
  body,
  {
    status = 200,
    headers = {
      "content-type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
    },
    redirected = false,
    type = "basic",
    url = "",
  } = {},
) {
  const retained = Uint8Array.from(body);
  let cancelled = false;
  return {
    status,
    headers: headerBag(headers),
    redirected,
    type,
    url,
    body: {
      getReader() {
        let delivered = false;
        return {
          async read() {
            if (delivered) {
              return { done: true, value: undefined };
            }
            delivered = true;
            return { done: false, value: Uint8Array.from(retained) };
          },
          releaseLock() {},
        };
      },
      async cancel() {
        cancelled = true;
      },
    },
    get cancelled() {
      return cancelled;
    },
  };
}

function scriptedFetch(script) {
  const calls = [];
  const fetch = async (url, options) => {
    calls.push({ url, options });
    if (script instanceof Error) {
      throw script;
    }
    return typeof script === "function" ? script(url, options) : script;
  };
  fetch.calls = calls;
  return fetch;
}

function client(fetch) {
  return new BootleLanternIssuanceClientV1({
    baseUrl: "https://torii.example",
    fetch,
  });
}

function credential() {
  return BootleLanternIssuanceCredentialV1.fromOpaqueBytes(
    Uint8Array.of(1, 2, 3),
  );
}

function errorFixtureBody(error) {
  return error.body_hex === undefined
    ? new TextEncoder().encode(error.body_utf8)
    : Uint8Array.from(Buffer.from(error.body_hex, "hex"));
}

function errorFixtureHeaders(error, bodyLength) {
  return {
    "content-type": [error.media_type],
    "content-length": [String(bodyLength)],
    ...(error.retry_after_seconds === undefined
      ? {}
      : { "retry-after": [String(error.retry_after_seconds)] }),
    ...(error.www_authenticate === undefined
      ? {}
      : { "www-authenticate": [error.www_authenticate] }),
  };
}

function errorFixtureResponse(error, overrides = {}) {
  const body = errorFixtureBody(error);
  const headers = errorFixtureHeaders(error, body.length);
  return response(body, {
    status: error.status,
    headers,
    ...overrides,
  });
}

function crc64Ecma(payload) {
  const mask = 0xffff_ffff_ffff_ffffn;
  const polynomial = 0xc96c_5795_d787_0f42n;
  let crc = mask;
  for (const byte of payload) {
    crc ^= BigInt(byte);
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc & 1n) === 0n
        ? crc >> 1n
        : polynomial ^ (crc >> 1n);
    }
  }
  return BigInt.asUintN(64, crc ^ mask);
}

function malformedNoritoFieldFrame(body) {
  const malformed = Uint8Array.from(body);
  assert.equal(Buffer.from(malformed.subarray(0, 4)).toString("ascii"), "NRT0");
  const view = new DataView(
    malformed.buffer,
    malformed.byteOffset,
    malformed.byteLength,
  );
  const payloadLength = Number(view.getBigUint64(23, true));
  assert.equal(40 + payloadLength, malformed.length);
  // Extend the first length-delimited struct field by one byte. The frame CRC
  // remains valid, so rejection exercises canonical ErrorEnvelope decoding.
  assert.ok(malformed[40] < 0x7f);
  malformed[40] += 1;
  view.setBigUint64(31, crc64Ecma(malformed.subarray(40)), true);
  return malformed;
}

function noritoFrameWithPayload(template, payload) {
  const frame = new Uint8Array(40 + payload.length);
  frame.set(template.subarray(0, 40));
  frame.set(payload, 40);
  const view = new DataView(frame.buffer, frame.byteOffset, frame.byteLength);
  view.setBigUint64(23, BigInt(payload.length), true);
  view.setBigUint64(31, crc64Ecma(payload), true);
  return frame;
}

function rejectedLegacyNoritoErrorFrame(template, code) {
  const text = new TextEncoder().encode(code);
  assert.ok(text.length < 0x80);
  // This was the pre-release, hand-framed shape: strings were placed directly
  // in the struct payload without their authoritative outer field envelopes.
  const payload = Uint8Array.from([
    text.length,
    ...text,
    text.length,
    ...text,
    0,
  ]);
  return noritoFrameWithPayload(template, payload);
}

test("shared client contract fixture binds exact wire bytes", async () => {
  assert.equal(
    clientContractFixture.schema,
    "iroha.bootle_lantern.issuance_client_contract",
  );
  assert.equal(clientContractFixture.version, 1);
  assert.equal(
    clientContractFixture.classification,
    "public-synthetic-test-data",
  );

  const { transport, credential: credentialContract, bodies, errors } =
    clientContractFixture;
  assert.equal(transport.method, "POST");
  assert.equal(
    transport.authorize_path,
    BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1,
  );
  assert.equal(
    transport.issue_path,
    BOOTLE_LANTERN_ISSUANCE_ISSUE_PATH_V1,
  );
  assert.equal(transport.norito_media_type, BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1);
  assert.equal(
    transport.unauthorized_www_authenticate,
    'Bearer realm="iroha-bootle-lantern-issuance"',
  );
  assert.equal(credentialContract.encoding, "base64url-unpadded-canonical");
  assert.equal(credentialContract.minimum_decoded_bytes, 1);
  assert.equal(
    credentialContract.maximum_decoded_bytes,
    BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1,
  );
  assert.equal(credentialContract.examples.length, 3);
  for (const example of credentialContract.examples) {
    const decoded = Uint8Array.from(Buffer.from(example.decoded_hex, "hex"));
    assert.equal(Buffer.from(decoded).toString("base64url"), example.encoded);
    const admitted =
      BootleLanternIssuanceCredentialV1.fromCanonicalBase64Url(example.encoded);
    const fetch = scriptedFetch(
      response(patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1)),
    );
    await client(fetch).authorize(admitted);
    assert.equal(
      fetch.calls[0].options.headers.Authorization,
      `Bearer ${example.encoded}`,
    );
    admitted.destroy();
  }

  assert.equal(
    bodies.pattern,
    "byte-at-index-equals-index-modulo-256-with-canonical-wire-magics",
  );
  for (const [name, wire, length] of [
    ["authorization_response", "ILA1", BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1],
    ["issue_request", "ILA1+ILQ1", BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1],
    ["issue_response", "ILR1", BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1],
  ]) {
    const body = bodies[name];
    assert.equal(body.wire, wire);
    assert.equal(body.length_bytes, length);
    assert.equal(
      createHash("sha256").update(patterned(length)).digest("hex"),
      body.pattern_sha256_hex,
    );
  }
  assert.equal(Buffer.from(patterned(320).subarray(0, 4)).toString(), "ILA1");
  assert.equal(Buffer.from(patterned(71_896).subarray(0, 4)).toString(), "ILA1");
  assert.equal(Buffer.from(patterned(71_896).subarray(320, 324)).toString(), "ILQ1");
  assert.equal(Buffer.from(patterned(3_176).subarray(0, 4)).toString(), "ILR1");
  assert.deepEqual(bodies.issue_request.component_lengths_bytes, [320, 71_576]);
  assert.equal(
    bodies.issue_request.component_lengths_bytes.reduce(
      (total, length) => total + length,
      0,
    ),
    BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1,
  );

  assert.equal(
    errors.maximum_body_bytes,
    BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1,
  );
  assert.deepEqual(errors.norito_envelope, {
    schema_type_name: "iroha_torii_shared::ErrorEnvelope",
    schema_hash_hex: "793f11768076bfe270a17aeb86752cd9",
    flags_hex: "02",
  });
  assert.equal(errors.responses.length, 8);
  for (const errorContract of errors.responses) {
    assert.equal(
      errorContract.www_authenticate,
      errorContract.status === 401
        ? transport.unauthorized_www_authenticate
        : undefined,
    );
    const fetch = scriptedFetch(errorFixtureResponse(errorContract));
    await assert.rejects(
      client(fetch).authorize(credential()),
      (error) => {
        assert.ok(error instanceof BootleLanternIssuanceClientErrorV1);
        assert.equal(error.status, errorContract.status);
        assert.equal(error.code, errorContract.code);
        assert.equal(
          error.retryAfterSeconds,
          errorContract.retry_after_seconds ?? null,
        );
        return true;
      },
    );
    assert.equal(fetch.calls.length, 1);
  }
});

test("authorize sends the canonical empty request exactly once", async () => {
  const output = patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1);
  const fetch = scriptedFetch(response(output));

  const actual = await client(fetch).authorize(
    BootleLanternIssuanceCredentialV1.fromOpaqueBytes(Uint8Array.of(0x61)),
  );

  assert.deepEqual(actual, output);
  assert.equal(fetch.calls.length, 1);
  const [{ url, options }] = fetch.calls;
  assert.equal(
    url,
    `https://torii.example${BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1}`,
  );
  assert.equal(options.method, "POST");
  assert.deepEqual(options.body, new Uint8Array(0));
  assert.equal(options.headers.Authorization, "Bearer YQ");
  assert.equal(
    options.headers["Content-Type"],
    BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1,
  );
  assert.equal(options.headers.Accept, BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1);
  assert.equal(options.headers["Accept-Encoding"], "identity");
  assert.equal(options.headers["Cache-Control"], "no-store");
  assert.equal(options.headers.Pragma, "no-cache");
  assert.equal(options.redirect, "manual");
  assert.equal(options.cache, "no-store");
  assert.equal(options.credentials, "omit");
});

test("issue defensively copies its exact request and response", async () => {
  const request = patterned(BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1);
  const expectedRequest = Uint8Array.from(request);
  const expectedResponse = patterned(BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1);
  let release;
  const gate = new Promise((resolve) => {
    release = resolve;
  });
  let observedRequest;
  const fetch = scriptedFetch(async (_url, options) => {
    observedRequest = options.body;
    await gate;
    return response(expectedResponse);
  });

  const pending = client(fetch).issue(credential(), request);
  request.fill(0);
  release();
  const actual = await pending;
  expectedResponse.fill(0);

  assert.deepEqual(observedRequest, expectedRequest);
  assert.notEqual(observedRequest, request);
  assert.equal(fetch.calls.length, 1);
  assert.equal(
    fetch.calls[0].url,
    `https://torii.example${BOOTLE_LANTERN_ISSUANCE_ISSUE_PATH_V1}`,
  );
  assert.equal(fetch.calls[0].options.headers.Authorization, "Bearer AQID");
  assert.deepEqual(actual, patterned(BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1));
});

test("issue rejects truncated, extended, empty, and wrong-typed bodies preflight", async () => {
  const fetch = scriptedFetch(
    response(patterned(BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1)),
  );
  const issuance = client(fetch);
  for (const size of [
    0,
    1,
    BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1 - 1,
    BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1 + 1,
    BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1 * 2,
  ]) {
    await assert.rejects(issuance.issue(credential(), new Uint8Array(size)), {
      name: "RangeError",
    });
  }
  await assert.rejects(issuance.issue(credential(), []), { name: "TypeError" });
  assert.equal(fetch.calls.length, 0);
});

test("issue rejects same-length wrong, truncated, shifted, and substituted ILA1 or ILQ1 magic preflight", async () => {
  const fetch = scriptedFetch(
    response(patterned(BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1)),
  );
  for (const prefix of [
    Uint8Array.of(0, 0, 0, 0),
    new TextEncoder().encode("ILA0"),
    Uint8Array.of(0x49, 0x4c, 0x41, 0),
    new TextEncoder().encode("XLA1"),
  ]) {
    const body = patterned(BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1);
    body.set(prefix, 0);
    await assert.rejects(client(fetch).issue(credential(), body), /ILA1 \|\| ILQ1/);
  }
  for (const prefix of [
    Uint8Array.of(0, 0, 0, 0),
    new TextEncoder().encode("ILQ0"),
    Uint8Array.of(0x49, 0x4c, 0x51, 0),
    new TextEncoder().encode("XLQ1"),
  ]) {
    const body = patterned(BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1);
    body.set(prefix, BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1);
    await assert.rejects(client(fetch).issue(credential(), body), /ILA1 \|\| ILQ1/);
  }
  assert.equal(fetch.calls.length, 0);
});

test("credentials are canonical, bounded, defensive, destroyable, and redacted", async () => {
  assert.throws(
    () => BootleLanternIssuanceCredentialV1.fromOpaqueBytes(new Uint8Array(0)),
    RangeError,
  );
  assert.throws(
    () =>
      BootleLanternIssuanceCredentialV1.fromOpaqueBytes(
        new Uint8Array(BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1 + 1),
      ),
    RangeError,
  );
  for (const malformed of [
    "",
    "A",
    "YQ==",
    "YR",
    "Y Q",
    "YQ\n",
    "Bearer YQ",
    "+w",
    "A".repeat(
      Math.ceil(BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1 / 3) * 4 + 1,
    ),
    Buffer.alloc(BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1 + 1).toString(
      "base64url",
    ),
  ]) {
    assert.throws(
      () =>
        BootleLanternIssuanceCredentialV1.fromCanonicalBase64Url(malformed),
      TypeError,
    );
  }

  const source = Uint8Array.of(0x61);
  const secret = BootleLanternIssuanceCredentialV1.fromOpaqueBytes(source);
  source[0] = 0x62;
  assert.equal(String(secret), "BootleLanternIssuanceCredentialV1([REDACTED])");
  const fetch = scriptedFetch(
    response(patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1)),
  );
  await client(fetch).authorize(secret);
  assert.equal(fetch.calls[0].options.headers.Authorization, "Bearer YQ");
  secret.destroy();
  secret.destroy();
  await assert.rejects(client(fetch).authorize(secret), /destroyed/);
  assert.equal(fetch.calls.length, 1);

  const maximum = new Uint8Array(
    BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1,
  ).fill(0xff);
  const maximumEncoded = Buffer.from(maximum).toString("base64url");
  const maximumCredential =
    BootleLanternIssuanceCredentialV1.fromCanonicalBase64Url(maximumEncoded);
  const maximumFetch = scriptedFetch(
    response(patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1)),
  );
  await client(maximumFetch).authorize(maximumCredential);
  assert.equal(
    maximumFetch.calls[0].options.headers.Authorization,
    `Bearer ${maximumEncoded}`,
  );
  maximumCredential.destroy();
});

test("authorization and issue responses require exact lengths", async () => {
  for (const size of [
    0,
    BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1 - 1,
    BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1 + 1,
  ]) {
    const fetch = scriptedFetch(response(patterned(size)));
    await assert.rejects(client(fetch).authorize(credential()), /exactly 320 bytes/);
    assert.equal(fetch.calls.length, 1);
  }
  for (const size of [
    0,
    BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1 - 1,
    BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1 + 1,
  ]) {
    const fetch = scriptedFetch(response(patterned(size)));
    await assert.rejects(
      client(fetch).issue(
        credential(),
        patterned(BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1),
      ),
      /exactly 3176 bytes/,
    );
    assert.equal(fetch.calls.length, 1);
  }
});

test("successful responses require exact ILA1 and ILR1 wire magic", async () => {
  for (const prefix of [
    Uint8Array.of(0, 0, 0, 0),
    new TextEncoder().encode("ILA0"),
    Uint8Array.of(0x49, 0x4c, 0x41, 0),
    new TextEncoder().encode("XLA1"),
  ]) {
    const body = patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1);
    body.set(prefix, 0);
    await assert.rejects(
      client(scriptedFetch(response(body))).authorize(credential()),
      /wire magic/,
    );
  }
  for (const prefix of [
    Uint8Array.of(0, 0, 0, 0),
    new TextEncoder().encode("ILR0"),
    Uint8Array.of(0x49, 0x4c, 0x52, 0),
    new TextEncoder().encode("XLR1"),
  ]) {
    const body = patterned(BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1);
    body.set(prefix, 0);
    await assert.rejects(
      client(scriptedFetch(response(body))).issue(
        credential(),
        patterned(BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1),
      ),
      /wire magic/,
    );
  }
});

test("responses require exact status and reject redirects", async () => {
  for (const status of [0, 201, 204, 301, 307, 308, 418, 500]) {
    const fetch = scriptedFetch(
      response(patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1), { status }),
    );
    await assert.rejects(
      client(fetch).authorize(credential()),
      BootleLanternIssuanceClientErrorV1,
    );
    assert.equal(fetch.calls.length, 1);
  }

  for (const options of [
    { redirected: true },
    { type: "opaqueredirect" },
    { url: "https://attacker.example/result" },
  ]) {
    const fetch = scriptedFetch(
      response(patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1), options),
    );
    await assert.rejects(client(fetch).authorize(credential()));
    assert.equal(fetch.calls.length, 1);
  }
});

test("structured errors bind status, media type, code, and retry hint", async () => {
  for (const errorContract of clientContractFixture.errors.responses) {
    const fetch = scriptedFetch(errorFixtureResponse(errorContract));
    await assert.rejects(
      client(fetch).authorize(credential()),
      (error) => {
        assert.ok(error instanceof BootleLanternIssuanceClientErrorV1);
        assert.equal(error.status, errorContract.status);
        assert.equal(error.code, errorContract.code);
        assert.equal(
          error.retryAfterSeconds,
          errorContract.retry_after_seconds ?? null,
        );
        return true;
      },
    );
  }
});

test("all seven Norito errors reject legacy, malformed, truncated, and trailing frames", async () => {
  const noritoErrors = clientContractFixture.errors.responses.filter(
    ({ media_type: mediaType }) =>
      mediaType === BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1,
  );
  assert.equal(noritoErrors.length, 7);

  for (const errorContract of noritoErrors) {
    const canonical = errorFixtureBody(errorContract);
    const variants = [
      rejectedLegacyNoritoErrorFrame(canonical, errorContract.code),
      malformedNoritoFieldFrame(canonical),
      canonical.subarray(0, canonical.length - 1),
      Uint8Array.from([...canonical, 0]),
    ];
    for (const body of variants) {
      const fetch = scriptedFetch(
        response(body, {
          status: errorContract.status,
          headers: errorFixtureHeaders(errorContract, body.length),
        }),
      );
      await assert.rejects(
        client(fetch).authorize(credential()),
        (error) => {
          assert.ok(error instanceof BootleLanternIssuanceClientErrorV1);
          assert.equal(error.status, null);
          assert.equal(error.code, null);
          assert.equal(error.retryAfterSeconds, null);
          return true;
        },
      );
      assert.equal(fetch.calls.length, 1);
    }
  }
});

test("structured errors reject malformed, substituted, and oversized envelopes", async () => {
  const contracts = clientContractFixture.errors.responses;
  const badRequest = contracts.find(({ status }) => status === 400);
  const unauthorized = contracts.find(({ status }) => status === 401);
  const notAcceptable = contracts.find(({ status }) => status === 406);
  const capacity = contracts.find(({ status }) => status === 429);
  const unavailable = contracts.find(({ status }) => status === 503);
  const challenge = unauthorized.www_authenticate;

  const corrupted = errorFixtureBody(badRequest);
  corrupted[0] ^= 1;
  const adversarial = [
    response(corrupted, {
      status: 400,
      headers: {
        "content-type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
        "content-length": [String(corrupted.length)],
      },
    }),
    errorFixtureResponse(badRequest, {
      headers: {
        "content-type": ["application/json"],
        "content-length": [String(errorFixtureBody(badRequest).length)],
      },
    }),
    errorFixtureResponse(badRequest, {
      headers: {
        "content-type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
        "content-encoding": ["identity"],
      },
    }),
    errorFixtureResponse(badRequest, {
      headers: {
        "content-type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
        "content-length": ["0107"],
      },
    }),
    response(errorFixtureBody(unauthorized), {
      status: 400,
      headers: {
        "content-type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
      },
    }),
    response(
      new TextEncoder().encode(`${notAcceptable.body_utf8} `),
      {
        status: 406,
        headers: { "content-type": ["application/json"] },
      },
    ),
    errorFixtureResponse(capacity, {
      headers: {
        "content-type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
        "retry-after": ["2"],
      },
    }),
    errorFixtureResponse(unavailable, {
      headers: {
        "content-type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
        "retry-after": ["1"],
      },
    }),
    errorFixtureResponse(unauthorized, {
      headers: {
        "content-type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
        "content-length": [String(errorFixtureBody(unauthorized).length)],
      },
    }),
    errorFixtureResponse(unauthorized, {
      headers: {
        "content-type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
        "content-length": [String(errorFixtureBody(unauthorized).length)],
        "www-authenticate": [challenge, challenge],
      },
    }),
    errorFixtureResponse(unauthorized, {
      headers: {
        "content-type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
        "content-length": [String(errorFixtureBody(unauthorized).length)],
        "www-authenticate": ['Bearer realm="attacker"'],
      },
    }),
    errorFixtureResponse(badRequest, {
      headers: {
        "content-type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
        "content-length": [String(errorFixtureBody(badRequest).length)],
        "www-authenticate": [challenge],
      },
    }),
    response(
      new Uint8Array(BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1 + 1),
      {
        status: 400,
        headers: {
          "content-type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
        },
      },
    ),
  ];
  for (const candidate of adversarial) {
    await assert.rejects(
      client(scriptedFetch(candidate)).authorize(credential()),
      (error) => {
        assert.ok(error instanceof BootleLanternIssuanceClientErrorV1);
        assert.equal(error.status, null);
        assert.equal(error.code, null);
        assert.equal(error.retryAfterSeconds, null);
        return true;
      },
    );
  }
});

test("responses reject missing, duplicated, and parameterized media types", async () => {
  for (const values of [
    [],
    ["Application/X-Norito"],
    ["application/octet-stream"],
    ["application/x-norito; charset=binary"],
    ["application/x-norito, application/x-norito"],
    ["application/x-norito", "application/x-norito"],
  ]) {
    const fetch = scriptedFetch(
      response(patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1), {
        headers: values.length === 0 ? {} : { "Content-Type": values },
      }),
    );
    await assert.rejects(client(fetch).authorize(credential()), /Content-Type/);
    assert.equal(fetch.calls.length, 1);
  }
});

test("responses reject compression and noncanonical or conflicting lengths", async () => {
  for (const encoding of [["gzip"], ["identity"], ["br"], ["gzip", "br"]]) {
    const fetch = scriptedFetch(
      response(patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1), {
        headers: {
          "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
          "Content-Encoding": encoding,
        },
      }),
    );
    await assert.rejects(client(fetch).authorize(credential()), /Content-Encoding/);
  }

  for (const length of [
    ["0"],
    ["319"],
    ["321"],
    ["0320"],
    ["+320"],
    ["320 "],
    ["320, 320"],
    ["320", "320"],
  ]) {
    const fetch = scriptedFetch(
      response(patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1), {
        headers: {
          "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
          "Content-Length": length,
        },
      }),
    );
    await assert.rejects(client(fetch).authorize(credential()), /Content-Length/);
  }

  const accepted = scriptedFetch(
    response(patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1), {
      headers: {
        "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
        "Content-Length": [String(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1)],
      },
    }),
  );
  await client(accepted).authorize(credential());

  const challenged = scriptedFetch(
    response(patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1), {
      headers: {
        "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
        "WWW-Authenticate": [
          'Bearer realm="iroha-bootle-lantern-issuance"',
        ],
      },
    }),
  );
  await assert.rejects(
    client(challenged).authorize(credential()),
    /unexpected WWW-Authenticate/,
  );
});

test("transport and asynchronous body failures are sanitized and never retried", async () => {
  const leaked = "opaque-secret-must-not-appear";
  const failedFetch = scriptedFetch(new Error(leaked));
  let transportError;
  await assert.rejects(
    client(failedFetch).authorize(credential()),
    (error) => {
      transportError = error;
      return error instanceof BootleLanternIssuanceClientErrorV1;
    },
  );
  assert.equal(failedFetch.calls.length, 1);
  assert.equal(String(transportError).includes(leaked), false);

  const bodyFetch = scriptedFetch({
    ...response(patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1)),
    body: {
      getReader() {
        return {
          async read() {
            throw new Error(leaked);
          },
          releaseLock() {},
        };
      },
      async cancel() {},
    },
  });
  let bodyError;
  await assert.rejects(
    client(bodyFetch).authorize(credential()),
    (error) => {
      bodyError = error;
      return error instanceof BootleLanternIssuanceClientErrorV1;
    },
  );
  assert.equal(bodyFetch.calls.length, 1);
  assert.equal(String(bodyError).includes(leaked), false);
});

test("responses without a bounded byte stream fail closed", async () => {
  const fetch = scriptedFetch({
    ...response(patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1)),
    body: null,
    async arrayBuffer() {
      return patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1).buffer;
    },
  });
  await assert.rejects(
    client(fetch).authorize(credential()),
    /not a bounded byte stream/,
  );
  assert.equal(fetch.calls.length, 1);
});

test("base URL admission rejects non-HTTPS, credentials, and non-origin state", () => {
  for (const baseUrl of [
    "",
    "torii.example",
    "http://torii.example",
    "https://user:secret@torii.example",
    "https://torii.example/v1",
    "https://torii.example/?",
    "https://torii.example/#",
    "https://torii.example/?query=1",
    "https://torii.example/#fragment",
  ]) {
    assert.throws(
      () => new BootleLanternIssuanceClientV1({ baseUrl, fetch: async () => {} }),
      TypeError,
    );
  }
});

test("request submission fails closed if the runtime strips identity encoding", async () => {
  const NativeRequest = globalThis.Request;
  globalThis.Request = class StrippingRequest {
    constructor(_url, options) {
      this.headers = {
        get(name) {
          if (name.toLowerCase() === "accept-encoding") {
            return null;
          }
          const entry = Object.entries(options.headers).find(
            ([candidate]) => candidate.toLowerCase() === name.toLowerCase(),
          );
          return entry?.[1] ?? null;
        },
      };
    }
  };
  const fetch = scriptedFetch(
    response(patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1)),
  );
  try {
    await assert.rejects(
      client(fetch).authorize(credential()),
      /cannot enforce canonical request headers/,
    );
    assert.equal(fetch.calls.length, 0);
  } finally {
    globalThis.Request = NativeRequest;
  }
});

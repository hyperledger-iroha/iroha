import assert from "node:assert/strict";
import { createServer } from "node:net";
import test from "node:test";

import {
  ToriiClient,
  ValidationErrorCode,
  canonicalRequestSignatureMessage,
  generateKeyPair,
  verifyEd25519,
} from "../src/index.js";
import { AccountAddress } from "../src/address.js";

test("ToriiClient attaches canonical signing headers for app endpoints", async () => {
  const captured = [];
  const fetchImpl = async (url, init) => {
    captured.push({ url, init });
    return new Response(JSON.stringify({ items: [], total: 0 }), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  };
  markFetchSupportsRawUtf8Headers(fetchImpl);
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl,
  });
  const { privateKey, publicKey } = generateKeyPair({ seed: Buffer.alloc(32, 9) });
  const accountId = AccountAddress.fromAccount({ publicKey }).toI105(369);

  await client.listAccountAssets(accountId, {
    canonicalAuth: { accountId, privateKey },
    limit: 1,
  });

  assert.equal(captured.length, 1);
  const { url, init } = captured[0];
  assert.equal(init.headers["X-Iroha-Account"], accountId);
  assert.deepEqual(init.__irohaRawUtf8Headers, {
    "X-Iroha-Account": accountId,
  });
  const signatureB64 = init.headers["X-Iroha-Signature"];
  assert.ok(typeof signatureB64 === "string" && signatureB64.length > 0);
  const timestampMs = Number(init.headers["X-Iroha-Timestamp-Ms"]);
  const nonce = init.headers["X-Iroha-Nonce"];
  assert.ok(Number.isFinite(timestampMs));
  assert.ok(typeof nonce === "string" && nonce.length > 0);

  const parsed = new URL(url);
  const message = canonicalRequestSignatureMessage({
    method: init.method,
    path: parsed.pathname,
    query: parsed.search ? parsed.search.slice(1) : "",
    body: "",
    timestampMs,
    nonce,
  });
  const signature = Buffer.from(signatureB64, "base64");
  assert.ok(verifyEd25519(message, signature, publicKey));
});

test("ToriiClient canonical auth falls back to a raw Node transport for UTF-8 account headers", async (t) => {
  const { privateKey, publicKey } = generateKeyPair({ seed: Buffer.alloc(32, 10) });
  const accountId = AccountAddress.fromAccount({ publicKey }).toI105(369);
  const requests = [];
  const server = createServer({ allowHalfOpen: true }, (socket) => {
    const chunks = [];
    let responded = false;
    const tryRespond = () => {
      if (responded) {
        return;
      }
      const request = Buffer.concat(chunks);
      const headerTerminator = request.indexOf(Buffer.from("\r\n\r\n", "ascii"));
      if (headerTerminator === -1) {
        return;
      }
      const headerBlock = request.subarray(0, headerTerminator).toString("latin1");
      const contentLengthMatch = /(?:^|\r\n)content-length:\s*(\d+)/iu.exec(headerBlock);
      const contentLength = contentLengthMatch ? Number.parseInt(contentLengthMatch[1], 10) : 0;
      const totalLength = headerTerminator + 4 + contentLength;
      if (request.length < totalLength) {
        return;
      }
      responded = true;
      requests.push(request.subarray(0, totalLength));
      const responseBody = Buffer.from(
        JSON.stringify({
          session_id: "sess_utf8",
          account_id: accountId,
          exit_class: "standard",
          relay_endpoint: "/dns/torii.exit.example/udp/9443/quic",
          lease_secs: 600,
          expires_at_ms: 1_700_000_000_000,
          connected_at_ms: 1_699_999_400_000,
          meter_family: "soranet.vpn.standard",
          route_pushes: [],
          excluded_routes: [],
          dns_servers: ["1.1.1.1"],
          tunnel_addresses: ["10.208.0.2/32"],
          mtu_bytes: 1280,
          helper_ticket_hex: "ab".repeat(32),
          bytes_in: 123,
          bytes_out: 456,
          status: "active",
        }),
        "utf8",
      );
      socket.end(
        Buffer.concat([
          Buffer.from(
            "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n",
            "ascii",
          ),
          Buffer.from(responseBody.length.toString(16), "ascii"),
          Buffer.from("\r\n", "ascii"),
          responseBody,
          Buffer.from("\r\n0\r\n\r\n", "ascii"),
        ]),
      );
    };
    socket.on("data", (chunk) => {
      chunks.push(Buffer.from(chunk));
      tryRespond();
    });
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  t.after(
    () =>
      new Promise((resolve, reject) => {
        server.close((error) => {
          if (error) {
            reject(error);
            return;
          }
          resolve();
        });
      }),
  );
  const address = server.address();
  assert(address && typeof address === "object");
  let fetchCalled = false;
  const client = new ToriiClient(`http://127.0.0.1:${address.port}`, {
    allowInsecure: true,
    fetchImpl: async () => {
      fetchCalled = true;
      throw new Error("fetch should not run");
    },
  });

  const session = await client.createVpnSession(
    { exitClass: "standard" },
    { canonicalAuth: { accountId, privateKey } },
  );

  assert.equal(fetchCalled, false);
  assert.equal(requests.length, 1);
  const request = requests[0];
  assert.ok(
    request.includes(Buffer.from(`X-Iroha-Account: ${accountId}\r\n`, "utf8")),
    "expected raw request to carry the UTF-8 account header bytes",
  );
  assert.ok(
    request.includes(Buffer.from("X-Iroha-Signature: ", "ascii")),
    "expected canonical signature header on the raw request",
  );
  const headerTerminator = request.indexOf(Buffer.from("\r\n\r\n", "ascii"));
  assert.notEqual(headerTerminator, -1);
  assert.deepEqual(
    JSON.parse(request.subarray(headerTerminator + 4).toString("utf8")),
    { exit_class: "standard" },
  );
  assert.equal(session.sessionId, "sess_utf8");
  assert.equal(session.accountId, accountId);
});

test("ToriiClient canonical auth rejects UTF-8 account headers when no supported transport is available", async () => {
  const client = new ToriiClient("wss://localhost:8080", {
    fetchImpl: async () =>
      new Response(JSON.stringify({ items: [], total: 0 }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
  });
  const { privateKey, publicKey } = generateKeyPair({ seed: Buffer.alloc(32, 11) });
  const accountId = AccountAddress.fromAccount({ publicKey }).toI105();

  await assert.rejects(
    () =>
      client.listAccountAssets(accountId, {
        canonicalAuth: { accountId, privateKey },
        limit: 1,
      }),
    (error) =>
      error?.name === "ValidationError" &&
      error?.code === ValidationErrorCode.INVALID_OBJECT &&
      error?.path === "canonicalAuth.accountId" &&
      /raw utf-8 header support/i.test(error.message),
  );
});

test("ToriiClient canonical auth accepts byte-array private keys", async () => {
  const captured = [];
  const fetchImpl = async (url, init) => {
    captured.push({ url, init });
    return new Response(JSON.stringify({ items: [], total: 0 }), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  };
  markFetchSupportsRawUtf8Headers(fetchImpl);
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl,
  });
  const { privateKey, publicKey } = generateKeyPair({ seed: Buffer.alloc(32, 3) });
  const accountId = AccountAddress.fromAccount({ publicKey }).toI105();

  await client.listAccountAssets(accountId, {
    canonicalAuth: { accountId, privateKey: Array.from(privateKey) },
    limit: 1,
  });

  assert.equal(captured.length, 1);
  const { url, init } = captured[0];
  const parsed = new URL(url);
  const timestampMs = Number(init.headers["X-Iroha-Timestamp-Ms"]);
  const nonce = init.headers["X-Iroha-Nonce"];
  assert.ok(Number.isFinite(timestampMs));
  assert.ok(typeof nonce === "string" && nonce.length > 0);
  const message = canonicalRequestSignatureMessage({
    method: init.method,
    path: parsed.pathname,
    query: parsed.search ? parsed.search.slice(1) : "",
    body: "",
    timestampMs,
    nonce,
  });
  const signature = Buffer.from(init.headers["X-Iroha-Signature"], "base64");
  assert.ok(verifyEd25519(message, signature, publicKey));
});

test("ToriiClient canonical auth rejects non-byte private key arrays", async () => {
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () =>
      new Response(JSON.stringify({ items: [], total: 0 }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
  });
  const { publicKey } = generateKeyPair({ seed: Buffer.alloc(32, 7) });
  const accountId = AccountAddress.fromAccount({ publicKey }).toI105();

  await assert.rejects(
    () =>
      client.listAccountAssets(accountId, {
        canonicalAuth: { accountId, privateKey: [256] },
        limit: 1,
      }),
    (error) => error?.name === "ValidationError" && /privateKey\[0\]/i.test(error.message),
  );
});

function markFetchSupportsRawUtf8Headers(fetchImpl) {
  fetchImpl.__irohaSupportsRawUtf8Headers = true;
  return fetchImpl;
}

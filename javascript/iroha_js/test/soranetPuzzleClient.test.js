import { test } from "node:test";
import assert from "node:assert/strict";
import { SoranetPuzzleClient, SoranetPuzzleError } from "../src/soranetPuzzleClient.js";

const BASE_URL = "http://localhost:8088";

function createFetch(queue) {
  return async (url, init) => {
    const next = queue.shift();
    if (!next) {
      throw new Error("unexpected fetch invocation");
    }
    next.capture?.(url, init);
    return next.response;
  };
}

function jsonResponse(status, body) {
  return {
    status,
    async text() {
      return JSON.stringify(body ?? {});
    },
  };
}

test("soranet puzzle client rejects fractional timeout", () => {
  assert.throws(
    () =>
      new SoranetPuzzleClient(BASE_URL, {
        fetchImpl: async () => jsonResponse(200, {}),
        timeoutMs: 1.5,
      }),
    /timeoutMs/,
  );
});

test("getPuzzleConfig normalises fields", async () => {
  const queue = [
    {
      response: jsonResponse(200, {
        required: true,
        difficulty: 8,
        max_future_skew_secs: 900,
        min_ticket_ttl_secs: 60,
        ticket_ttl_secs: 120,
        puzzle: { memory_kib: 4096, time_cost: 3, lanes: 4 },
        token: {
          enabled: true,
          suite: "ML-DSA-44",
          relay_id_hex: "aa".repeat(32),
          issuer_fingerprint_hex: "bb".repeat(32),
          max_ttl_secs: 600,
          min_ttl_secs: 60,
          default_ttl_secs: 300,
          clock_skew_secs: 30,
          revocation_ids_hex: ["cc".repeat(32)],
        },
      }),
    },
  ];
  const client = new SoranetPuzzleClient(BASE_URL, {
    fetchImpl: createFetch(queue),
  });
  const snapshot = await client.getPuzzleConfig();
  assert.equal(snapshot.required, true);
  assert.equal(snapshot.puzzle?.memoryKib, 4096);
  assert.equal(snapshot.token.revocationIdsHex.length, 1);
});

test("mintPuzzleTicket passes overrides", async () => {
  let captured;
  const queue = [
    {
      capture(url, init) {
        captured = { url, init };
      },
      response: jsonResponse(200, {
        ticket_b64: "Zm9v",
        difficulty: 5,
        ttl_secs: 120,
        expires_at: 1_700_000_000,
      }),
    },
  ];
  const client = new SoranetPuzzleClient(BASE_URL, {
    fetchImpl: createFetch(queue),
  });
  const result = await client.mintPuzzleTicket({ ttlSecs: 90 });
  assert.equal(result.ticketB64, "Zm9v");
  assert.equal(result.signedTicketB64, null);
  assert.equal(result.signedTicketFingerprintHex, null);
  assert.equal(captured.url, `${BASE_URL}/v1/puzzle/mint`);
  assert.equal(JSON.parse(captured.init.body).ttl_secs, 90);
});

test("mintPuzzleTicket requests signed tickets with transcript binding", async () => {
  let captured;
  const queue = [
    {
      capture(url, init) {
        captured = { url, init };
      },
      response: jsonResponse(200, {
        ticket_b64: "Zm9v",
        signed_ticket_b64: "YmFy",
        signed_ticket_fingerprint_hex: "11".repeat(32),
        difficulty: 5,
        ttl_secs: 120,
        expires_at: 1_700_000_000,
      }),
    },
  ];
  const client = new SoranetPuzzleClient(BASE_URL, {
    fetchImpl: createFetch(queue),
  });
  const result = await client.mintPuzzleTicket({
    ttlSecs: 90,
    transcriptHashHex: "aa".repeat(32),
    signed: true,
  });
  const body = JSON.parse(captured.init.body);
  assert.equal(body.signed, true);
  assert.equal(body.transcript_hash_hex, "aa".repeat(32));
  assert.equal(result.signedTicketB64, "YmFy");
  assert.equal(result.signedTicketFingerprintHex, "11".repeat(32));
});

test("mintAdmissionToken validates hex + propagates TTL + flags", async () => {
  const queue = [
    {
      response: jsonResponse(200, {
        token_b64: "YmFy",
        token_id_hex: "11".repeat(32),
        issued_at: 10,
        expires_at: 20,
        ttl_secs: 10,
        flags: 1,
        issuer_fingerprint_hex: "22".repeat(32),
        relay_id_hex: "33".repeat(32),
      }),
    },
  ];
  const client = new SoranetPuzzleClient(BASE_URL, {
    fetchImpl: createFetch(queue),
  });
  const token = await client.mintAdmissionToken("44".repeat(32), {
    ttlSecs: 30,
    flags: 2,
    issuedAtUnix: 5,
  });
  assert.equal(token.tokenB64, "YmFy");
});

test("request throws SoranetPuzzleError on failure", async () => {
  const queue = [
    {
      response: {
        status: 500,
        async text() {
          return "boom";
        },
      },
    },
  ];
  const client = new SoranetPuzzleClient(BASE_URL, {
    fetchImpl: createFetch(queue),
  });
  await assert.rejects(
    () => client.getPuzzleConfig(),
    (error) => error instanceof SoranetPuzzleError && error.body === "boom",
  );
});

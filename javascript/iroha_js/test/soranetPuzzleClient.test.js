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

test("soranet puzzle client rejects the retired fetch option alias", () => {
  assert.throws(
    () =>
      new SoranetPuzzleClient(BASE_URL, {
        fetch: async () => jsonResponse(200, {}),
      }),
    /options\.fetch.*first-release API/,
  );
});

test("getPuzzleConfig normalises fields", async () => {
  let captured;
  const queue = [
    {
      capture(url, init) {
        captured = { url, init };
      },
      response: jsonResponse(200, {
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
  assert.equal(snapshot.puzzle.memoryKib, 4096);
  assert.equal(snapshot.token.revocationIdsHex.length, 1);
  assert.equal(captured.url, `${BASE_URL}/v1/puzzle/config`);
  assert.equal(captured.init.body, undefined);
  assert.equal(captured.init.headers["Content-Type"], undefined);
});

test("getPuzzleConfig rejects a missing mandatory puzzle", async () => {
  const queue = [
    {
      response: jsonResponse(200, {
        difficulty: 8,
        max_future_skew_secs: 900,
        min_ticket_ttl_secs: 60,
        ticket_ttl_secs: 120,
        token: { enabled: false, revocation_ids_hex: [] },
      }),
    },
  ];
  const client = new SoranetPuzzleClient(BASE_URL, {
    fetchImpl: createFetch(queue),
  });
  await assert.rejects(() => client.getPuzzleConfig(), /puzzle.*must be an object/);
});

test("getPuzzleConfig rejects zero-work puzzle policy", async () => {
  const client = new SoranetPuzzleClient(BASE_URL, {
    fetchImpl: async () =>
      jsonResponse(200, {
        difficulty: 0,
        max_future_skew_secs: 900,
        min_ticket_ttl_secs: 60,
        ticket_ttl_secs: 120,
        puzzle: { memory_kib: 4096, time_cost: 3, lanes: 4 },
        token: { enabled: false, revocation_ids_hex: [] },
      }),
  });
  await assert.rejects(() => client.getPuzzleConfig(), /difficulty.*greater than zero/);
});

test("getPuzzleConfig rejects retired admission toggles", async () => {
  for (const retiredField of ["required", "signed_ticket_signing_enabled"]) {
    const queue = [
      {
        response: jsonResponse(200, {
          [retiredField]: true,
          difficulty: 8,
          max_future_skew_secs: 900,
          min_ticket_ttl_secs: 60,
          ticket_ttl_secs: 120,
          puzzle: { memory_kib: 4096, time_cost: 3, lanes: 4 },
          token: { enabled: false, revocation_ids_hex: [] },
        }),
      },
    ];
    const client = new SoranetPuzzleClient(BASE_URL, {
      fetchImpl: createFetch(queue),
    });
    await assert.rejects(
      () => client.getPuzzleConfig(),
      new RegExp(`${retiredField}.*first-release API`),
    );
  }
});

test("mintPuzzleTicket passes overrides", async () => {
  let captured;
  const queue = [
    {
      capture(url, init) {
        captured = { url, init };
      },
      response: jsonResponse(200, {
        credential_kind: "raw",
        credential_b64: "Zm9v",
        difficulty: 5,
        ttl_secs: 120,
        expires_at: 1_700_000_000,
      }),
    },
  ];
  const client = new SoranetPuzzleClient(BASE_URL, {
    fetchImpl: createFetch(queue),
  });
  const result = await client.mintPuzzleTicket("99".repeat(32), { ttlSecs: 90 });
  assert.equal(result.credentialKind, "raw");
  assert.equal(result.credentialB64, "Zm9v");
  assert.equal(result.signedTicketFingerprintHex, null);
  assert.equal(captured.url, `${BASE_URL}/v1/puzzle/mint`);
  const body = JSON.parse(captured.init.body);
  assert.equal(body.ttl_secs, 90);
  assert.equal(body.transcript_hash_hex, "99".repeat(32));
});

test("mintPuzzleTicket accepts a server-selected signed credential", async () => {
  let captured;
  const queue = [
    {
      capture(url, init) {
        captured = { url, init };
      },
      response: jsonResponse(200, {
        credential_kind: "signed",
        credential_b64: "YmFy",
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
  const result = await client.mintPuzzleTicket("aa".repeat(32), { ttlSecs: 90 });
  const body = JSON.parse(captured.init.body);
  assert.equal(result.credentialKind, "signed");
  assert.equal(result.credentialB64, "YmFy");
  assert.equal(Object.hasOwn(body, "signed"), false);
  assert.equal(body.transcript_hash_hex, "aa".repeat(32));
  assert.equal(result.signedTicketFingerprintHex, "11".repeat(32));
});

test("mintPuzzleTicket rejects the retired signed request option", async () => {
  const client = new SoranetPuzzleClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("request must not be sent");
    },
  });
  await assert.rejects(
    () => client.mintPuzzleTicket("aa".repeat(32), { signed: true }),
    /signed.*first-release API/,
  );
});

test("mintPuzzleTicket rejects retired dual-field credential responses", async () => {
  const queue = [
    {
      response: jsonResponse(200, {
        ticket_b64: "Zm9v",
        signed_ticket_b64: "YmFy",
        difficulty: 5,
        ttl_secs: 120,
        expires_at: 1_700_000_000,
      }),
    },
  ];
  const client = new SoranetPuzzleClient(BASE_URL, {
    fetchImpl: createFetch(queue),
  });
  await assert.rejects(
    () => client.mintPuzzleTicket("ab".repeat(32)),
    /ticket_b64.*first-release API/,
  );
});

test("mintPuzzleTicket rejects a signed-ticket fingerprint for raw credentials", async () => {
  const queue = [
    {
      response: jsonResponse(200, {
        credential_kind: "raw",
        credential_b64: "Zm9v",
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
  await assert.rejects(
    () => client.mintPuzzleTicket("ab".repeat(32)),
    /only valid for signed credentials/,
  );
});

test("mintPuzzleTicket requires a fingerprint for signed credentials", async () => {
  const queue = [
    {
      response: jsonResponse(200, {
        credential_kind: "signed",
        credential_b64: "YmFy",
        difficulty: 5,
        ttl_secs: 120,
        expires_at: 1_700_000_000,
      }),
    },
  ];
  const client = new SoranetPuzzleClient(BASE_URL, {
    fetchImpl: createFetch(queue),
  });
  await assert.rejects(
    () => client.mintPuzzleTicket("ab".repeat(32)),
    /required for signed credentials/,
  );
});

test("mintPuzzleTicket rejects missing or zero transcript binding", async () => {
  const client = new SoranetPuzzleClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("request must not be sent");
    },
  });
  await assert.rejects(() => client.mintPuzzleTicket(), /transcriptHashHex/);
  await assert.rejects(
    () => client.mintPuzzleTicket("00".repeat(32)),
    /must not be all zeros/,
  );
});

test("mintAdmissionToken validates hex and propagates TTL", async () => {
  let captured;
  const queue = [
    {
      capture(url, init) {
        captured = { url, init };
      },
      response: jsonResponse(200, {
        token_b64: "YmFy",
        token_id_hex: "11".repeat(32),
        issued_at: 10,
        expires_at: 20,
        ttl_secs: 10,
        issuer_fingerprint_hex: "22".repeat(32),
        relay_id_hex: "33".repeat(32),
      }),
    },
  ];
  const client = new SoranetPuzzleClient(BASE_URL, {
    fetchImpl: createFetch(queue),
  });
  const token = await client.mintAdmissionToken("44".repeat(32), { ttlSecs: 30 });
  assert.equal(token.tokenB64, "YmFy");
  assert.deepEqual(JSON.parse(captured.init.body), {
    transcript_hash_hex: "44".repeat(32),
    ttl_secs: 30,
  });
});

test("mintAdmissionToken rejects the retired flags option", async () => {
  const client = new SoranetPuzzleClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("request must not be sent");
    },
  });
  await assert.rejects(
    () => client.mintAdmissionToken("44".repeat(32), { flags: 1 }),
    /flags.*first-release API/,
  );
});

test("mintAdmissionToken rejects a retired flags response field", async () => {
  const queue = [
    {
      response: jsonResponse(200, {
        token_b64: "YmFy",
        token_id_hex: "11".repeat(32),
        issued_at: 10,
        expires_at: 20,
        ttl_secs: 10,
        flags: 0,
        issuer_fingerprint_hex: "22".repeat(32),
        relay_id_hex: "33".repeat(32),
      }),
    },
  ];
  const client = new SoranetPuzzleClient(BASE_URL, {
    fetchImpl: createFetch(queue),
  });
  await assert.rejects(
    () => client.mintAdmissionToken("44".repeat(32)),
    /flags.*first-release API/,
  );
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

test("timed requests remove external abort listeners after completion", async () => {
  let added = 0;
  let removed = 0;
  const externalSignal = {
    aborted: false,
    addEventListener(type) {
      assert.equal(type, "abort");
      added += 1;
    },
    removeEventListener(type) {
      assert.equal(type, "abort");
      removed += 1;
    },
  };
  const client = new SoranetPuzzleClient(BASE_URL, {
    timeoutMs: 1_000,
    fetchImpl: async () =>
      jsonResponse(200, {
        difficulty: 1,
        max_future_skew_secs: 900,
        min_ticket_ttl_secs: 60,
        ticket_ttl_secs: 120,
        puzzle: { memory_kib: 4096, time_cost: 1, lanes: 1 },
        token: { enabled: false, revocation_ids_hex: [] },
      }),
  });

  await client.getPuzzleConfig({ signal: externalSignal });
  assert.equal(added, 1);
  assert.equal(removed, 1);
});

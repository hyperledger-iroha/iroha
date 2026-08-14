import { readFileSync } from "node:fs";

const DIST_SOURCE_BYTE_MISMATCH_MESSAGE =
  "dist/toriiClient.js must exactly match src/toriiClient.js; rebuild the distribution bundle";

function assertExactDistributionBytes(assert, actual, expected) {
  assert.equal(Buffer.compare(actual, expected), 0, DIST_SOURCE_BYTE_MISMATCH_MESSAGE);
}

/** Register bounded diagnostics for stale distribution bytes. */
export function registerToriiClientDistributionMemoryTests({ assert, test }) {
  test("SoraFS orderbook canonical dist matches the reviewed source", () => {
    const distribution = readFileSync(new URL("../dist/toriiClient.js", import.meta.url));
    const source = readFileSync(new URL("../src/toriiClient.js", import.meta.url));
    assertExactDistributionBytes(assert, distribution, source);
  });

  test("SoraFS distribution byte assertion has a bounded stale-bundle diagnostic", () => {
    const source = Buffer.alloc(1024 * 1024, 0xa5);
    const distribution = Buffer.from(source);
    distribution[distribution.length - 1] ^= 0xff;

    assert.throws(
      () => assertExactDistributionBytes(assert, distribution, source),
      (error) => {
        assert.ok(error.message.startsWith(DIST_SOURCE_BYTE_MISMATCH_MESSAGE));
        assert.ok(error.message.length < 128);
        return true;
      },
    );
  });
}

/** Register bounded operator-response transport regressions. */
export function registerToriiClientBoundedResponseTests({
  assert,
  BASE_URL,
  createResponse,
  ISO_OPERATOR_SIGNING_CONTEXT,
  test,
  ToriiClient,
}) {
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

  test("operator diagnostic JSON reads reject oversized declared bodies before parsing", async () => {
    const cases = [
      {
        name: "pipeline recovery",
        maximumBodyBytes: 8 * 1024 * 1024,
        invoke: (client) => client.getPipelineRecovery(1),
      },
      {
        name: "pipeline preflight",
        maximumBodyBytes: 64 * 1024 * 1024,
        invoke: (client) => client.getPipelinePreflight(),
      },
      {
        name: "pipeline recovery FASTPQ proofs",
        maximumBodyBytes: 24 * 1024 * 1024,
        invoke: (client) => client.getPipelineRecoveryFastpqProofs(1),
      },
      {
        name: "network time status",
        maximumBodyBytes: 64 * 1024 * 1024,
        invoke: (client) => client.getNetworkTimeStatus(),
      },
      {
        name: "peer list",
        maximumBodyBytes: 64 * 1024 * 1024,
        invoke: (client) => client.listPeers(),
      },
    ];

    for (const { name, maximumBodyBytes, invoke } of cases) {
      let cancelCalls = 0;
      let jsonCalls = 0;
      const client = new ToriiClient(BASE_URL, {
        fetchImpl: async () => ({
          status: 200,
          headers: new Headers({
            "content-type": "application/json",
            "content-length": String(maximumBodyBytes + 1),
          }),
          body: {
            cancel() {
              cancelCalls += 1;
            },
          },
          async json() {
            jsonCalls += 1;
            throw new Error("bounded readers must not call Response.json");
          },
        }),
      });
      await assert.rejects(invoke(client), /response limit/u, name);
      assert.equal(cancelCalls, 1, `${name} must cancel before reading`);
      assert.equal(jsonCalls, 0, `${name} must not call Response.json`);
    }
  });

  test("operator diagnostic error-body reads honor caller abort", async () => {
    const cases = [
      ["pipeline recovery", (client, signal) => client.getPipelineRecovery(1, { signal })],
      ["pipeline preflight", (client, signal) => client.getPipelinePreflight({ signal })],
      [
        "pipeline recovery FASTPQ proofs",
        (client, signal) => client.getPipelineRecoveryFastpqProofs(1, { signal }),
      ],
      ["network time status", (client, signal) => client.getNetworkTimeStatus({ signal })],
      ["peer list", (client, signal) => client.listPeers({ signal })],
    ];

    for (const [name, invoke] of cases) {
      let cancelCalls = 0;
      let markReadStarted;
      const readStarted = new Promise((resolve) => {
        markReadStarted = resolve;
      });
      const body = new ReadableStream({
        pull() {
          markReadStarted();
          return new Promise(() => {});
        },
        cancel() {
          cancelCalls += 1;
        },
      });
      const client = new ToriiClient(BASE_URL, {
        fetchImpl: async () => new Response(body, {
          status: 500,
          headers: { "content-type": "application/json" },
        }),
      });
      const controller = new AbortController();
      const pending = invoke(client, controller.signal);
      await readStarted;
      controller.abort();
      await assert.rejects(
        pending,
        (error) => error?.name === "AbortError",
        `${name} must propagate caller abort while reading an error body`,
      );
      assert.equal(cancelCalls, 1, `${name} must cancel its stalled error body`);
    }
  });

  test("missing recovery responses cancel unread bodies", async () => {
    const cases = [
      ["pipeline recovery", (client) => client.getPipelineRecovery(1)],
      [
        "pipeline recovery FASTPQ proofs",
        (client) => client.getPipelineRecoveryFastpqProofs(1),
      ],
    ];

    for (const [name, invoke] of cases) {
      let cancelCalls = 0;
      const client = new ToriiClient(BASE_URL, {
        fetchImpl: async () => ({
          status: 404,
          body: {
            cancel() {
              cancelCalls += 1;
            },
          },
        }),
      });
      assert.equal(await invoke(client), null, name);
      assert.equal(cancelCalls, 1, `${name} must cancel its unread 404 body`);
    }
  });

  test("missing recovery responses propagate aborts after cleanup", async () => {
    const cases = [
      ["pipeline recovery", (client, signal) => client.getPipelineRecovery(1, { signal })],
      [
        "pipeline recovery FASTPQ proofs",
        (client, signal) => client.getPipelineRecoveryFastpqProofs(1, { signal }),
      ],
    ];

    for (const [name, invoke] of cases) {
      const controller = new AbortController();
      const abortReason = new Error(`${name} cancelled`);
      let cancelCalls = 0;
      const client = new ToriiClient(BASE_URL, {
        fetchImpl: async () => {
          controller.abort(abortReason);
          return {
            status: 404,
            body: {
              cancel() {
                cancelCalls += 1;
              },
            },
          };
        },
      });

      await assert.rejects(invoke(client, controller.signal), (error) => error === abortReason);
      assert.equal(cancelCalls, 1, `${name} must cancel before propagating abort`);
    }
  });

  test("pipeline recovery bounds streamed bytes despite missing or lying Content-Length", async () => {
    const maximumBodyBytes = 8 * 1024 * 1024;
    for (const declaredLength of [null, "1"]) {
      let cancelled = false;
      const body = new ReadableStream({
        start(controller) {
          controller.enqueue(new Uint8Array(maximumBodyBytes));
          controller.enqueue(Uint8Array.of(0x20));
        },
        cancel() {
          cancelled = true;
        },
      });
      const headers = { "content-type": "application/json" };
      if (declaredLength !== null) headers["content-length"] = declaredLength;
      const client = new ToriiClient(BASE_URL, {
        fetchImpl: async () => new Response(body, { status: 200, headers }),
      });
      await assert.rejects(
        () => client.getPipelineRecovery(1),
        /exceeds the 8388608-byte response limit/u,
      );
      assert.equal(cancelled, true);
    }
  });

  test("FASTPQ recovery sends fresh operator authentication", async () => {
    let requestHeaders;
    const client = new ToriiClient(BASE_URL, {
      fetchImpl: async (_url, init) => {
        requestHeaders = new Headers(init.headers);
        return createResponse({
          status: 200,
          jsonData: {},
          headers: { "content-type": "application/json" },
        });
      },
    });

    await client.getPipelineRecoveryFastpqProofs(1);
    assert.match(
      requestHeaders.get("x-iroha-operator-signature"),
      /^[A-Za-z0-9+/]+={0,2}$/u,
    );
    assert.equal(
      requestHeaders.get("x-iroha-operator-public-key"),
      ISO_OPERATOR_SIGNING_CONTEXT.publicKey,
    );
  });

  test("pipeline recovery caller abort cancels its bounded body read", async () => {
    const controller = new AbortController();
    let cancelled = false;
    const body = new ReadableStream({
      pull() {
        return new Promise(() => {});
      },
      cancel() {
        cancelled = true;
      },
    });
    const client = new ToriiClient(BASE_URL, {
      fetchImpl: async () => new Response(body, {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    });

    const pending = client.getPipelineRecovery(1, { signal: controller.signal });
    const abortReason = new Error("custom recovery cancellation");
    controller.abort(abortReason);
    await assert.rejects(pending, (error) => error === abortReason);
    assert.equal(cancelled, true);
  });
}

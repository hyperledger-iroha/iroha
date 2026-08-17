import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";
import {
  browserSumeragiDiagnosticsFixture,
  browserSumeragiStatusFixture,
} from "./sumeragiBrowserFixtures.js";

export const SUMERAGI_DIAGNOSTICS_CONTRACT_TESTS = Object.freeze([
  "getSumeragiStatus validates options",
  "getSumeragiStatus fetches the flattened v2 payload without rewriting it",
  "typed Sumeragi endpoints reject swapped status and diagnostics payloads",
  "getSumeragiStatusTyped preserves exact u64 tokens from the raw HTTP body",
  "getSumeragiDiagnosticsTyped preserves Native application u64 boundaries",
  "getSumeragiDiagnosticsTyped preserves exact u64 Native AMX V2 receipt identities",
  "getSumeragiDiagnosticsTyped rejects non-u64 Native integer spellings",
  "typed Sumeragi JSON rejects duplicate keys, trailing input, and oversized bodies",
  "getSumeragiDiagnosticsTyped parses bounded native application evidence and enforces state geometry",
  "getSumeragiDiagnosticsTyped rejects native application evidence above the server bound",
  "getSumeragiDiagnosticsTyped requires the autonomous execution vector",
  "getSumeragiDiagnosticsTyped parses autonomous execution stages and explicit conflict",
  "getSumeragiDiagnosticsTyped requires exact provisional identity hashes",
  "getSumeragiDiagnosticsTyped enforces reservation-only geometry",
  "getSumeragiDiagnosticsTyped pairs finalized identity and orders by provisional identity",
  "getSumeragiStatusTyped validates and normalizes authoritative v2 status",
  "getSumeragiStatusTyped accepts a non-empty Native AMX application manifest",
  "getSumeragiStatusTyped rejects invalid Native AMX application manifests",
  "getSumeragiStatusTyped requires exact lane-finality and merge projections",
  "getSumeragiStatusTyped requires an exact executed block wire length",
  "Sumeragi execution commitment declarations expose current mandatory fields",
  "getSumeragiStatusTyped preserves exact proposal rounds",
  "getSumeragiStatusTyped enforces vote-quorum proposal geometry",
  "getSumeragiStatusTyped enforces outbound-intent proposal geometry",
  "getSumeragiStatusTyped accepts the local-control liveness blocker",
  "getSumeragiStatusTyped accepts the unsafe-proposal ignore reason",
  "getSumeragiStatusTyped accepts all twelve ignore reasons at the bound",
  "getSumeragiStatusTyped rejects unsupported protocol and invalid frozen contexts",
  "getSumeragiStatusTyped rejects malformed liveness diagnostics",
  "retired aggregate Sumeragi telemetry, RBC, and collector helpers are absent",
  "getSumeragiStatusTyped rejects inconsistent or under-quorum commits",
  "getSumeragiDiagnosticsTyped rejects impossible queue snapshots",
  "getSumeragiDiagnosticsTyped requires every canonical lane array",
  "getSumeragiDiagnosticsTyped parses exact nested fee and native AMX receipts",
  "getSumeragiDiagnosticsTyped accepts the canonical first participant-lane block",
  "getSumeragiDiagnosticsTyped accepts mixed-role proposals without the current entrypoint",
  "getSumeragiDiagnosticsTyped keeps global and coordinator views independent",
  "getSumeragiDiagnosticsTyped rejects unordered native QC validators",
  "getSumeragiDiagnosticsTyped rejects invalid and identity BLS-Normal validators",
  "getSumeragiDiagnosticsTyped rejects participant-finality tampering",
  "getSumeragiDiagnosticsTyped rejects non-canonical settlement scalars and nested fields",
  "getSumeragiDiagnosticsTyped rejects nested receipt identity and QC tampering",
  "getSumeragiDiagnosticsTyped enforces bounded lane observability before nested decode",
  "getSumeragiDiagnosticsTyped rejects adversarial lane evidence",
]);

const focusSymbol = Symbol.for("iroha.js.test.sumeragiDiagnosticsContract");
assert.equal(
  Object.hasOwn(globalThis, focusSymbol),
  false,
  "Sumeragi diagnostics focus selector must have one owner",
);

const selectedClientPath =
  process.env.IROHA_JS_SUMERAGI_DIAGNOSTICS_TORII_CLIENT ?? "";
let clientModuleUrl;
if (selectedClientPath === "") {
  clientModuleUrl = new URL("../src/toriiClient.js", import.meta.url);
} else {
  assert.equal(
    path.isAbsolute(selectedClientPath),
    true,
    "focused Sumeragi diagnostics client path must be absolute",
  );
  clientModuleUrl = pathToFileURL(selectedClientPath);
}
const { ToriiClient } = await import(clientModuleUrl.href);
assert.equal(typeof ToriiClient, "function");
const clientIndexModuleUrl = new URL("./index.js", clientModuleUrl);
const { ValidationError } = await import(clientIndexModuleUrl.href);
assert.equal(typeof ValidationError, "function");
const browserClientModuleUrl = new URL("./toriiBrowserClient.js", clientModuleUrl);
const { ToriiBrowserClient } = await import(browserClientModuleUrl.href);
assert.equal(typeof ToriiBrowserClient, "function");

async function verifyTypedBrowserSurface() {
  const diagnosticsText = JSON.stringify(browserSumeragiDiagnosticsFixture())
    .replace(
      '"tx_queue_retained_bytes":4096',
      '"tx_queue_retained_bytes":9007199254740993',
    )
    .replace(
      '"tx_queue_max_retained_bytes":65536',
      '"tx_queue_max_retained_bytes":9007199254740994',
    );
  const requests = [];
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async (url, init) => {
      requests.push([String(url), init.method, init.headers.Accept]);
      const body = String(url).endsWith("/v1/sumeragi/status")
        ? JSON.stringify(browserSumeragiStatusFixture())
        : diagnosticsText;
      return new Response(body, {
        headers: { "content-type": "application/json; charset=utf-8" },
      });
    },
  });
  const status = await client.getSumeragiStatusTyped();
  const diagnostics = await client.getSumeragiDiagnosticsTyped();
  assert.equal(status.protocol_version, 4);
  assert.equal(diagnostics.tx_queue_retained_bytes, 9007199254740993n);
  assert.equal(diagnostics.tx_queue_max_retained_bytes, 9007199254740994n);
  assert.deepEqual(requests, [
    [
      "https://torii.example/v1/sumeragi/status",
      "GET",
      "application/json",
    ],
    [
      "https://torii.example/v1/sumeragi/diagnostics",
      "GET",
      "application/json",
    ],
  ]);

  const rawPayload = { operational_note: "raw payload" };
  const separationClient = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => new Response(JSON.stringify(rawPayload), {
      headers: { "content-type": "application/json" },
    }),
  });
  assert.deepEqual(await separationClient.getSumeragiStatus(), rawPayload);
  assert.deepEqual(await separationClient.getSumeragiDiagnostics(), rawPayload);
  await assert.rejects(
    separationClient.getSumeragiStatusTyped(),
    /unknown field/u,
  );
  await assert.rejects(
    separationClient.getSumeragiDiagnosticsTyped(),
    /unknown field/u,
  );

  const declaredLengths = [1024 * 1024 + 1, 16 * 1024 * 1024 + 1];
  const boundedClient = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => new Response("{}", {
      headers: {
        "content-length": String(declaredLengths.shift()),
        "content-type": "application/json",
      },
    }),
  });
  await assert.rejects(
    boundedClient.getSumeragiStatusTyped(),
    /1048576-byte response limit/u,
  );
  await assert.rejects(
    boundedClient.getSumeragiDiagnosticsTyped(),
    /16777216-byte response limit/u,
  );

  const validStatus = JSON.stringify(browserSumeragiStatusFixture());
  const strictResponses = [
    new Response(`{"protocol_version":4,${validStatus.slice(1)}`, {
      headers: { "content-type": "application/json" },
    }),
    new Response(validStatus, {
      headers: { "content-type": "text/plain" },
    }),
  ];
  const strictClient = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => strictResponses.shift(),
  });
  await assert.rejects(
    strictClient.getSumeragiStatusTyped(),
    /duplicate object key/u,
  );
  await assert.rejects(
    strictClient.getSumeragiStatusTyped(),
    /application\/json media type/u,
  );

  const invalidUtf8Responses = [
    new Response(Uint8Array.of(0xff), {
      headers: { "content-type": "application/json" },
    }),
    new Response(Uint8Array.of(0xff), {
      headers: { "content-type": "application/json" },
    }),
  ];
  const invalidUtf8Client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => invalidUtf8Responses.shift(),
  });
  await assert.rejects(
    invalidUtf8Client.getSumeragiStatusTyped(),
    /must be valid UTF-8/u,
  );
  await assert.rejects(
    invalidUtf8Client.getSumeragiDiagnosticsTyped(),
    /must be valid UTF-8/u,
  );
}

await verifyTypedBrowserSurface();

const focus = {
  names: new Set(SUMERAGI_DIAGNOSTICS_CONTRACT_TESTS),
  observed: [],
  ToriiClient,
  ValidationError,
};
assert.equal(focus.names.size, SUMERAGI_DIAGNOSTICS_CONTRACT_TESTS.length);
globalThis[focusSymbol] = focus;
try {
  await import("./toriiClient.test.js?sumeragi-diagnostics-contract=1");
} finally {
  delete globalThis[focusSymbol];
}

assert.deepEqual(
  focus.observed,
  SUMERAGI_DIAGNOSTICS_CONTRACT_TESTS,
  "focused Sumeragi diagnostics test registrations must match the exact inventory",
);

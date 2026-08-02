import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

export const SUMERAGI_DIAGNOSTICS_CONTRACT_TESTS = Object.freeze([
  "getSumeragiStatus validates options",
  "getSumeragiStatus fetches the flattened v2 payload without rewriting it",
  "typed Sumeragi endpoints reject swapped status and diagnostics payloads",
  "getSumeragiStatusTyped preserves exact u64 tokens from the raw HTTP body",
  "getSumeragiDiagnosticsTyped preserves Native application u64 boundaries",
  "getSumeragiDiagnosticsTyped preserves exact u64 Native AMX V2 receipt identities",
  "getSumeragiDiagnosticsTyped rejects non-u64 Native integer spellings",
  "typed Sumeragi JSON rejects duplicate keys, trailing input, and oversized bodies",
  "getSumeragiDiagnosticsTyped parses bounded native application evidence",
  "getSumeragiDiagnosticsTyped rejects native application evidence above the server bound",
  "getSumeragiDiagnosticsTyped requires the autonomous execution vector",
  "getSumeragiDiagnosticsTyped parses autonomous execution stages and explicit conflict",
  "getSumeragiDiagnosticsTyped requires exact provisional identity hashes",
  "getSumeragiDiagnosticsTyped enforces reservation-only geometry",
  "getSumeragiDiagnosticsTyped pairs finalized identity and orders by provisional identity",
  "getSumeragiStatusTyped validates and normalizes authoritative v2 status",
  "getSumeragiStatusTyped accepts a non-empty Native AMX application manifest",
  "getSumeragiStatusTyped rejects invalid Native AMX application manifests",
  "getSumeragiStatusTyped requires an exact merge carrier projection",
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
  "retired global Sumeragi RBC and collector helpers are absent",
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

const focus = {
  names: new Set(SUMERAGI_DIAGNOSTICS_CONTRACT_TESTS),
  observed: [],
  ToriiClient,
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

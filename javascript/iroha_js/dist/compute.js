import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { blake2b256 } from "./blake2b.js";

const MODULE_DIR = path.dirname(fileURLToPath(import.meta.url));
const WORKSPACE_ROOT = path.resolve(MODULE_DIR, "../../..");

const DEFAULT_MANIFEST = path.resolve(
  WORKSPACE_ROOT,
  "fixtures/compute/manifest_compute_payments.json",
);
const DEFAULT_CALL = path.resolve(WORKSPACE_ROOT, "fixtures/compute/call_compute_payments.json");
const DEFAULT_PAYLOAD = path.resolve(
  WORKSPACE_ROOT,
  "fixtures/compute/payload_compute_payments.json",
);

function crc16(tag, body) {
  let crc = 0xffff;
  const processByte = (byte) => {
    crc ^= (byte & 0xff) << 8;
    for (let i = 0; i < 8; i += 1) {
      if ((crc & 0x8000) !== 0) {
        crc = ((crc << 1) ^ 0x1021) & 0xffff;
      } else {
        crc = (crc << 1) & 0xffff;
      }
    }
  };

  for (const byte of Buffer.from(tag, "utf8")) {
    processByte(byte);
  }
  processByte(":".charCodeAt(0));
  for (const byte of Buffer.from(body, "utf8")) {
    processByte(byte);
  }

  return crc & 0xffff;
}

function canonicalHashLiteral(buf) {
  const normalized = Buffer.from(buf);
  if (normalized.length !== 32) {
    throw new Error("hash must be 32 bytes");
  }
  normalized[normalized.length - 1] |= 1;
  const body = normalized.toString("hex").toUpperCase();
  const checksum = crc16("hash", body).toString(16).toUpperCase().padStart(4, "0");
  return `hash:${body}#${checksum}`;
}

function parseJson(pathname, description) {
  const raw = fs.readFileSync(pathname, "utf8");
  try {
    return JSON.parse(raw);
  } catch (error) {
    throw new Error(`failed to parse ${description} at ${pathname}: ${error.message}`);
  }
}

function loadPayload(pathname) {
  const raw = fs.readFileSync(pathname, "utf8").trim();
  return Buffer.from(raw, "utf8");
}

export function loadComputeFixtures(options = {}) {
  const manifestPath = options.manifestPath ?? DEFAULT_MANIFEST;
  const callPath = options.callPath ?? DEFAULT_CALL;
  const payloadPath = options.payloadPath ?? DEFAULT_PAYLOAD;
  return {
    manifest: parseJson(manifestPath, "compute manifest"),
    call: parseJson(callPath, "compute call"),
    payload: loadPayload(payloadPath),
    paths: { manifestPath, callPath, payloadPath },
  };
}

export function computePayloadHashLiteral(payload) {
  return canonicalHashLiteral(blake2b256(payload));
}

export function validatePayloadHash(fixtures) {
  const expected = fixtures.call?.request?.payload_hash;
  const actual = computePayloadHashLiteral(fixtures.payload);
  if (expected && expected !== actual) {
    throw new Error(
      `payload hash mismatch: expected ${expected}, computed ${actual} from ${fixtures.paths.payloadPath}`,
    );
  }
  return actual;
}

export function simulateCompute(fixtures) {
  const routeId = fixtures.call?.route;
  const route =
    fixtures.manifest?.routes?.find(
      (candidate) =>
        candidate.id?.service === routeId?.service && candidate.id?.method === routeId?.method,
    ) ?? null;
  if (!route) {
    throw new Error("compute route missing from manifest");
  }
  validatePayloadHash(fixtures);

  const { response, outcome } = executeEntrypoint(
    route.entrypoint,
    fixtures.payload,
    fixtures.call,
  );
  const responseHash = response ? canonicalHashLiteral(blake2b256(response)) : null;
  const responseB64 = response ? Buffer.from(response).toString("base64") : null;
  return {
    route,
    outcome,
    response,
    responseHash,
    responseB64,
  };
}

export function buildGatewayRequest(fixtures) {
  validatePayloadHash(fixtures);
  return {
    namespace: fixtures.call.namespace,
    codec: fixtures.call.codec,
    ttl_slots: fixtures.call.ttl_slots,
    gas_limit: fixtures.call.gas_limit,
    max_response_bytes: fixtures.call.max_response_bytes,
    determinism: fixtures.call.determinism,
    execution_class: fixtures.call.execution_class,
    declared_input_bytes: fixtures.call.declared_input_bytes,
    declared_input_chunks: fixtures.call.declared_input_chunks,
    sponsor_budget_cu: fixtures.call.sponsor_budget_cu,
    price_family: fixtures.call.price_family,
    resource_profile: fixtures.call.resource_profile,
    auth: fixtures.call.auth,
    headers: fixtures.call.request?.headers ?? {},
    payload_b64: Buffer.from(fixtures.payload).toString("base64"),
  };
}

function executeEntrypoint(entrypoint, payload, call) {
  let response;
  switch (entrypoint) {
    case "echo":
      response = Buffer.from(payload);
      break;
    case "uppercase":
      response = Buffer.from(payload.toString("utf8").toUpperCase(), "utf8");
      break;
    case "sha3": {
      const digest = Buffer.from(blake2b256(payload));
      digest[digest.length - 1] |= 1;
      response = digest;
      break;
    }
    default:
      throw new Error(`unsupported compute entrypoint ${entrypoint}`);
  }
  const cycles = payload.length * 10_000 + 5_000;
  if (cycles > call.gas_limit) {
    response = null;
  }
  if (response && response.length > call.max_response_bytes) {
    response = null;
  }
  return {
    response,
    outcome: response ? "Success" : "BudgetExhausted",
  };
}

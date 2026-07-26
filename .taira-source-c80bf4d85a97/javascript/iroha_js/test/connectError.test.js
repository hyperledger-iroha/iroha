import { test } from "node:test";
import assert from "node:assert/strict";

import {
  ConnectError,
  ConnectErrorCategory,
  ConnectQueueError,
  connectErrorFrom,
} from "../src/connectError.js";

test("connectErrorFrom returns existing ConnectError", () => {
  const base = new ConnectError({
    category: ConnectErrorCategory.TRANSPORT,
    code: "client.closed",
    message: "session closed",
    fatal: true,
  });
  const result = connectErrorFrom(base);
  assert.strictEqual(result, base, "existing ConnectError should pass through");
});

test("connect queue overflow maps to queueOverflow category", () => {
  const overflow = ConnectQueueError.overflow(128);
  const converted = overflow.toConnectError();
  assert.equal(converted.category, ConnectErrorCategory.QUEUE_OVERFLOW);
  assert.equal(converted.code, "queue.overflow");
  assert.ok(
    converted.underlying?.includes("limit=128"),
    "overflow metadata must include limit",
  );
  const telemetry = converted.telemetryAttributes();
  assert.equal(telemetry.category, "queueOverflow");
  assert.equal(telemetry.code, "queue.overflow");
  assert.equal(telemetry.fatal, "false");
});

test("connect queue expiration maps to timeout category", () => {
  const expired = ConnectQueueError.expired(5_000);
  const converted = connectErrorFrom(expired);
  assert.equal(converted.category, ConnectErrorCategory.TIMEOUT);
  assert.equal(converted.code, "queue.expired");
  assert.ok(
    converted.underlying?.includes("ttlMs=5000"),
    "expired metadata must include ttl",
  );
});

test("http status errors derive authorization category", () => {
  const httpErr = Object.assign(new Error("Forbidden"), { status: 403 });
  const converted = connectErrorFrom(httpErr);
  assert.equal(converted.category, ConnectErrorCategory.AUTHORIZATION);
  assert.equal(converted.code, "http.forbidden");
  assert.equal(converted.httpStatus, 403);
});

test("network socket failures map to transport category", () => {
  const netErr = Object.assign(new Error("connection reset"), {
    code: "ECONNRESET",
  });
  const converted = connectErrorFrom(netErr);
  assert.equal(converted.category, ConnectErrorCategory.TRANSPORT);
  assert.equal(converted.code, "client.closed");
});

test("tls failures map to authorization category", () => {
  const tlsErr = Object.assign(new Error("hostname mismatch"), {
    code: "ERR_TLS_CERT_ALTNAME_INVALID",
  });
  const converted = connectErrorFrom(tlsErr);
  assert.equal(converted.category, ConnectErrorCategory.AUTHORIZATION);
  assert.equal(converted.code, "network.tls_failure");
});

test("timeout detection handles timeouts codes and names", () => {
  const timeoutErr = Object.assign(new Error("timeout"), {
    code: "ETIMEDOUT",
  });
  const converted = connectErrorFrom(timeoutErr);
  assert.equal(converted.category, ConnectErrorCategory.TIMEOUT);
  assert.equal(converted.code, "network.timeout");
});

test("syntax errors surface codec category", () => {
  const err = new SyntaxError("Unexpected token } in JSON");
  const converted = connectErrorFrom(err);
  assert.equal(converted.category, ConnectErrorCategory.CODEC);
  assert.equal(converted.code, "codec.syntax_error");
});

test("http timeout status maps to timeout category", () => {
  const httpErr = Object.assign(new Error("Request Timeout"), { status: 408 });
  const converted = connectErrorFrom(httpErr);
  assert.equal(converted.category, ConnectErrorCategory.TIMEOUT);
  assert.equal(converted.code, "http.timeout");
  assert.equal(converted.httpStatus, 408);
});

test("http rate limit status maps to retryable transport category", () => {
  const httpErr = Object.assign(new Error("Too Many Requests"), { status: 429 });
  const converted = connectErrorFrom(httpErr);
  assert.equal(converted.category, ConnectErrorCategory.TRANSPORT);
  assert.equal(converted.code, "http.rate_limited");
  assert.equal(converted.httpStatus, 429);
});

test("http 4xx client errors no longer map to authorization by default", () => {
  const httpErr = Object.assign(new Error("Not Found"), { status: 404 });
  const converted = connectErrorFrom(httpErr);
  assert.equal(converted.category, ConnectErrorCategory.TRANSPORT);
  assert.equal(converted.code, "http.client_error");
  assert.equal(converted.httpStatus, 404);
});

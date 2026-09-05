// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";

import { Kagemusha } from "../src/kagemusha.js";

const fixture = JSON.parse(readFileSync(
  new URL("../../../fixtures/offline/kagemusha_v1.json", import.meta.url),
  "utf8",
));

function raw(section) {
  return Uint8Array.from(Buffer.from(section.norito_hex, "hex"));
}

function assertCanonicalSection(section, kind, value, encoder, ...bindings) {
  const expected = raw(section);
  assert.deepEqual(Buffer.from(encoder(value, ...bindings)), Buffer.from(expected));
  assert.equal(createHash("sha256").update(expected).digest("hex"), section.sha256);
  assert.equal(Kagemusha.encodeText(kind, expected), section.kgm1);
  assert.deepEqual(Buffer.from(Kagemusha.decodeText(kind, section.kgm1)), Buffer.from(expected));
  assert.equal(section.raw_bytes, expected.length);
}

test("all SDKs consume the Rust-generated canonical KAGEMUSHA three-message fixture", () => {
  assert.equal(fixture.fixture_version, 1);
  assert.equal(fixture.protocol, "KAGEMUSHA");
  assert.equal(fixture.text_prefix, "kgm1:");
  assert.deepEqual(fixture.ipm1_message_order, [
    { kind: "request", tag: 1 },
    { kind: "payment", tag: 2 },
    { kind: "acknowledgement", tag: 3 },
  ]);
  assert.equal("acceptance_intent" in fixture, false);
  assert.equal("acceptance_ticket" in fixture, false);

  const requestRaw = raw(fixture.payment_request);
  const request = Kagemusha.decodePaymentRequest(requestRaw);
  const paymentRaw = raw(fixture.payment);
  const payment = Kagemusha.decodePayment(paymentRaw, request);
  const acknowledgementRaw = raw(fixture.acknowledgement);
  const acknowledgement = Kagemusha.decodeAcknowledgement(
    acknowledgementRaw, request, payment,
  );

  assertCanonicalSection(
    fixture.payment_request, "paymentRequest", request, Kagemusha.encodePaymentRequest,
  );
  assertCanonicalSection(
    fixture.payment, "payment", payment, Kagemusha.encodePayment, request,
  );
  assertCanonicalSection(
    fixture.acknowledgement, "acknowledgement", acknowledgement,
    Kagemusha.encodeAcknowledgement, request, payment,
  );

  const expectedRaw = requestRaw.length + paymentRaw.length + acknowledgementRaw.length;
  assert.equal(Kagemusha.validateCompleteExchange(request, payment, acknowledgement), expectedRaw);
  assert.equal(fixture.complete_exchange.raw_bytes, expectedRaw);
  assert.deepEqual(fixture.complete_exchange.messages, [
    "request", "payment", "acknowledgement",
  ]);
  assert.equal(
    fixture.complete_exchange.text_bytes,
    fixture.payment_request.kgm1.length + fixture.payment.kgm1.length
      + fixture.acknowledgement.kgm1.length,
  );
});

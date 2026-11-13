// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

// Torii attaches `details` to queue rejections and to every HTTP 429 and 503.
// The shared decoders must read the Rust-authored frames and JSON renderings in
// `fixtures/torii/error_envelope_details_v1.json` exactly, leave an envelope
// whose details carry a structured observation to its caller, and refuse every
// other encoding of an otherwise valid envelope.
import assert from "node:assert/strict";
import test from "node:test";

import {
  decodeCanonicalJsonErrorEnvelope,
  decodeCanonicalNoritoErrorEnvelope,
  decodeTypedToriiErrorEnvelope,
} from "../src/toriiErrorEnvelope.js";
import {
  NORITO_NONE,
  errorDetailsPayload,
  errorEnvelopeDetailsContract as contract,
  errorEnvelopeFrame,
  errorEnvelopeFrameFromPayload,
  errorEnvelopePayload,
  leb128,
  noritoField,
  noritoSome,
  noritoString,
  noritoUnsigned,
} from "./helpers/toriiErrorEnvelope.js";

const NORITO = "application/x-norito";
const JSON_TYPE = "application/json; charset=utf-8";
const row = (name) => contract.decoded.find((entry) => entry.name === name);
const frameOf = (entry) => Buffer.from(entry.norito_hex, "hex");
const jsonOf = (entry) => Buffer.from(entry.json_utf8, "utf8");

test("details frames and JSON renderings decode exactly as Rust authored them", () => {
  assert.equal(contract.norito_envelope.schema_type_name, "iroha_torii_shared::ErrorEnvelope");
  assert.equal(contract.decoded.length, 8);
  for (const entry of contract.decoded) {
    const expected = { code: entry.code, message: entry.message, details: entry.details };
    assert.deepEqual(errorEnvelopeFrame(entry.code, entry.message, entry.details), frameOf(entry), entry.name);
    assert.deepEqual(decodeCanonicalNoritoErrorEnvelope(frameOf(entry)), expected, entry.name);
    assert.deepEqual(decodeCanonicalJsonErrorEnvelope(jsonOf(entry)), expected, entry.name);
    assert.deepEqual(decodeTypedToriiErrorEnvelope(NORITO, frameOf(entry)), { ...expected, format: "norito" });
    assert.deepEqual(decodeTypedToriiErrorEnvelope(JSON_TYPE, jsonOf(entry)), { ...expected, format: "json" });
  }
  // Unsigned integers are numbers up to 2^53 - 1 and exact bigints beyond.
  const { details } = decodeCanonicalNoritoErrorEnvelope(frameOf(row("every_decoded_field")));
  assert.equal(details.retry_after_seconds, Number.MAX_SAFE_INTEGER);
  assert.equal(details.queue.capacity, 2n ** 53n);
  assert.equal(details.queue.queued, 2n ** 64n - 1n);
  assert.equal(details.axt.lane, 2 ** 32 - 1);
  assert.equal(details.chain_discriminant, 65_535);
  assert.equal(details.hint, "");
  assert.deepEqual(decodeCanonicalNoritoErrorEnvelope(frameOf(row("empty_details"))).details, {});
});

test("an envelope whose details carry a structured observation is left to the caller", () => {
  assert.equal(contract.undecoded.length, 2);
  for (const entry of contract.undecoded) {
    assert.equal(decodeTypedToriiErrorEnvelope(NORITO, frameOf(entry)), null, entry.name);
    assert.equal(decodeTypedToriiErrorEnvelope(JSON_TYPE, jsonOf(entry)), null, entry.name);
    assert.throws(() => decodeCanonicalNoritoErrorEnvelope(frameOf(entry)), /observation/u);
    assert.throws(() => decodeCanonicalJsonErrorEnvelope(jsonOf(entry)), /observation/u);
  }
});

test("Norito details refuse every non-canonical encoding", () => {
  const code = "queue_full";
  const message = "transaction queue is at capacity";
  const details = row("queue_full").details;
  const decodes = (payload, flags) =>
    decodeTypedToriiErrorEnvelope(NORITO, errorEnvelopeFrameFromPayload(payload, flags)) !== null;
  const withDetails = (raw) => errorEnvelopePayload(code, message, details, raw);
  const queue = (saturatedByte) => noritoSome(Buffer.concat([
    noritoField(noritoString("saturated")),
    noritoField(noritoUnsigned(65_536, 8)),
    noritoField(noritoUnsigned(65_536, 8)),
    noritoField(Buffer.of(saturatedByte)),
  ]));
  // The builder reproduces the Rust frame, so each case below differs from a
  // canonical frame only by the named defect (its checksum stays valid).
  assert.deepEqual(errorEnvelopeFrameFromPayload(errorEnvelopePayload(code, message, details)), frameOf(row("queue_full")));
  assert.equal(decodes(withDetails({ queue: queue(1) })), true);
  const details26 = errorDetailsPayload(details);
  const cases = {
    "header flags without COMPACT_LEN": [errorEnvelopePayload(code, message, details), 0x00],
    "details option tag 2": [Buffer.concat([
      noritoField(noritoString(code)), noritoField(noritoString(message)),
      noritoField(Buffer.concat([Buffer.of(2), leb128(details26.length), details26])),
    ])],
    "absent details with a trailing byte": [Buffer.concat([
      noritoField(noritoString(code)), noritoField(noritoString(message)), noritoField(Buffer.of(0, 0)),
    ])],
    "an empty option field": [withDetails({ layer: Buffer.alloc(0) })],
    "absent option with a trailing byte": [withDetails({ layer: Buffer.of(0, 0) })],
    "a bool byte of 2": [withDetails({ queue: queue(2) })],
    "a seven-byte u64": [withDetails({ retry_after_seconds: noritoSome(noritoUnsigned(1, 7)) })],
    "a nine-byte u64": [withDetails({ retry_after_seconds: noritoSome(noritoUnsigned(1, 9)) })],
    "a four-byte u16": [withDetails({ chain_discriminant: noritoSome(noritoUnsigned(1, 4)) })],
    "an overlong field length": [Buffer.concat([
      noritoField(noritoString(code)), noritoField(noritoString(message)),
      noritoField(noritoSome(Buffer.concat([Buffer.of(0x81, 0x00), NORITO_NONE, details26.subarray(2)]))),
    ])],
    "trailing bytes after a string": [withDetails({ layer: noritoSome(Buffer.concat([noritoString("torii"), Buffer.of(0)])) })],
    "an invalid UTF-8 string": [withDetails({ reject_code: noritoSome(Buffer.of(1, 0xff)) })],
    "a present observation": [withDetails({ ivm_fault: noritoSome(noritoString("fault")) })],
    "a missing details field": [Buffer.concat([
      noritoField(noritoString(code)), noritoField(noritoString(message)),
      noritoField(noritoSome(details26.subarray(0, details26.length - 2))),
    ])],
    "an extra details field": [Buffer.concat([
      noritoField(noritoString(code)), noritoField(noritoString(message)),
      noritoField(noritoSome(Buffer.concat([details26, noritoField(NORITO_NONE)]))),
    ])],
    "an empty code": [errorEnvelopePayload("", message, details)],
  };
  for (const [label, [payload, flags]] of Object.entries(cases)) {
    assert.equal(decodes(payload, flags), false, label);
  }
  // A leading U+FEFF is text, not a byte-order mark.
  assert.equal(decodeCanonicalNoritoErrorEnvelope(errorEnvelopeFrame(code, "﻿m")).message, "﻿m");
});

test("JSON details refuse every non-canonical rendering", () => {
  const decodes = (text) => decodeTypedToriiErrorEnvelope(JSON_TYPE, Buffer.from(text, "utf8"));
  const envelope = (details) => `{"code":"c","message":"m","details":${details}}`;
  assert.deepEqual(decodes(`{"code":"c","message":"m"}`), { code: "c", message: "m", details: null, format: "json" });
  assert.deepEqual(decodes(envelope("{}")).details, {});
  assert.deepEqual(decodes(envelope(`{"reject_code":"R","retry_after_seconds":1}`)).details,
    { reject_code: "R", retry_after_seconds: 1 });
  assert.equal(decodes(envelope(`{"chain_discriminant":65535}`)).details.chain_discriminant, 65_535);
  assert.equal(decodes(envelope(`{"axt":{"lane":4294967295}}`)).details.axt.lane, 4_294_967_295);
  assert.equal(decodes(envelope(`{"retry_after_seconds":18446744073709551615}`)).details.retry_after_seconds,
    2n ** 64n - 1n);
  assert.equal(decodes(`{"code":"c","message":"﻿m"}`).message, "﻿m");
  const queueFull = row("queue_full").json_utf8;
  const nonCanonical = [
    queueFull.replace(`"code":`, `"code": `),
    `${queueFull}\n`,
    `﻿${queueFull}`,
    `{"message":"m","code":"c"}`,
    `{"code":"c","message":"m",}`,
    `{"code":"","message":"m"}`,
    `{"code":"c","message":"m","details":null}`,
    envelope(`{"retry_after_seconds":1,"reject_code":"R"}`),
    envelope(`{"retry_after_secs":1}`),
    envelope(`{"layer":null}`),
    envelope(`{"layer":"a","layer":"b"}`),
    envelope(`{"retry_after_seconds":01}`),
    envelope(`{"retry_after_seconds":1.0}`),
    envelope(`{"retry_after_seconds":1e0}`),
    envelope(`{"retry_after_seconds":-1}`),
    envelope(`{"retry_after_seconds":"1"}`),
    envelope(`{"retry_after_seconds":18446744073709551616}`),
    envelope(`{"chain_discriminant":65536}`),
    envelope(`{"axt":{"lane":4294967296}}`),
    envelope(`{"queue":{"state":"healthy","queued":1,"capacity":2,"saturated":1}}`),
    envelope(`{"queue":{"state":"healthy","queued":1,"capacity":2}}`),
    envelope(`{"fee":{"code":"vault_insufficient"}}`),
    envelope(`{"ivm_fault":{}}`),
    envelope(`{"account_alias_not_found":{"alias":"a@b"}}`),
    `{"code":"c","message":"\\u001F"}`,
    `{"code":"c","message":"\\/"}`,
    `{"code":"c","message":"\\u0061"}`,
    `{"code":"c","message":"\\ud83c\\udf38"}`,
    `{"code":"c","message":"\\ud800"}`,
    `{"code":"c","message":"\u0001"}`,
    `{"code":"c","message":"m"`,
  ];
  for (const text of nonCanonical) {
    assert.equal(decodes(text), null, text);
  }
  assert.equal(decodeTypedToriiErrorEnvelope(JSON_TYPE, Buffer.of(0x7b, 0xff, 0x7d)), null);
});

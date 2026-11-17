import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import { Buffer } from "buffer";
import * as codec from "../src/noritoValueCodecs.js";
import { createNoritoRecordDecoder, createNoritoRecordEncoder } from "../src/noritoRecordDecoder.js";
import { noritoDecodeBlockProofs, decodeRetailFeeAssessmentMarkerMessage } from "../src/norito.js";
import { decodeClassedRaceFrameV1 } from "../src/classedRace.js";
import { normalizeParliamentAttemptReadResponseV1 } from "../src/parliamentApiV1.js";

const vectors = JSON.parse(fs.readFileSync(new URL("../../../fixtures/norito_dense/vectors.json", import.meta.url), "utf8"));
const context = "dense vector";
const recordDecode = createNoritoRecordDecoder(codec.decodeStructFields, codec.decodeOptionValue, codec.decodeNoritoVec);
const recordEncode = createNoritoRecordEncoder(codec.encodeStructValue, codec.encodeOptionValue, codec.encodeNoritoVec);
const pair = (encode, decode) => ({ encode, decode });
const bytes = (length) => pair(codec.encodeNoritoByteArray, (value) => codec.decodeNoritoByteArray(value, length, context));
const string = pair(codec.encodeNoritoStringValue, (value) => codec.decodeStringValue(value, context));
const vec = (item) => pair(
  (value) => codec.encodeNoritoVec(value, item.encode),
  (value) => codec.decodeNoritoVec(value, item.decode, context),
);
const option = (item) => pair(
  (value) => codec.encodeOptionValue(value, item.encode, context),
  (value) => codec.decodeOptionValue(value, item.decode, context),
);
const scalar = (size) => pair(
  size === 4 ? (value) => codec.encodeU32Value(value, context) : codec.u64ToLittleEndianBuffer,
  (value) => {
    const reader = new codec.BufferReader(value, context);
    const result = size === 4 ? reader.readU32LE("value") : reader.readU64LE("value");
    reader.assertEof();
    return result;
  },
);
const choice = pair(
  ({ tag, value }) => codec.encodeEnumTagValue(tag, tag === 0 ? undefined : () => (tag === 1 ? string : scalar(8)).encode(value)),
  (value) => {
    const reader = new codec.BufferReader(value, context);
    const tag = reader.readEnumTag("tag");
    assert.ok([0, 1, 300].includes(tag));
    const decoded = tag === 0 ? null : (tag === 1 ? string : scalar(8)).decode(codec.readNoritoField(reader, "value"));
    reader.assertEof();
    return { tag, value: decoded };
  },
);
const map = pair(
  (entries) => Buffer.concat([codec.encodeNoritoCount(entries.length), ...entries.flatMap(([key, value]) => [codec.encodeNoritoField(string.encode(key)), codec.encodeNoritoField(codec.encodeU16Value(value, context))])]),
  (value) => {
    const reader = new codec.BufferReader(value, context);
    const count = reader.readCount("count"), entries = [];
    for (let i = 0; i < count; i += 1) {
      entries.push([string.decode(codec.readNoritoField(reader, "key")), codec.decodeU16Value(codec.readNoritoField(reader, "value"), context)]);
    }
    reader.assertEof();
    return entries;
  },
);
const result = pair(
  ({ tag, value }) => Buffer.concat([Buffer.of(tag), codec.encodeNoritoField(codec.encodeU8Value(value, context))]),
  (value) => {
    const reader = new codec.BufferReader(value, context), tag = reader.readU8("tag");
    assert.ok(tag === 0 || tag === 1);
    const decoded = codec.decodeU8Value(codec.readNoritoField(reader, "value"), context);
    reader.assertEof();
    return { tag, value: decoded };
  },
);
const raw = (length) => pair((value) => Buffer.from(value), (value) => { assert.equal(value.length, length); return Buffer.from(value); });
const record = (fields) => {
  const schema = fields.map(([name, item]) => [name, item.decode, 0, item.encode, 0]);
  return pair((value) => recordEncode(value, context, schema), (value) => recordDecode(value, context, schema));
};
const recordType = record([["id", scalar(4)], ["key", raw(4)], ["digests", vec(bytes(4))], ["label", option(string)], ["choice", choice]]);
const byteVec = pair(codec.encodeNoritoByteVec, (value) => codec.decodeNoritoByteVec(value, context));
const cases = {
  u8_vec: [byteVec, Buffer.of(1, 2, 3)],
  u8_vec_128: [byteVec, Buffer.alloc(128, 0x11)],
  u8_array: [bytes(4), Buffer.of(1, 2, 3, 4)],
  option_u8_array: [option(bytes(4)), Buffer.alloc(4, 9)],
  option_none: [option(bytes(4)), null],
  vec_u8_array: [vec(bytes(2)), [Buffer.alloc(2, 7), Buffer.alloc(2, 8)]],
  vec_string: [vec(string), ["ab"]],
  vec_string_empty: [vec(string), []],
  map_string_u16: [map, [["k", 5]]],
  enum_unit: [choice, { tag: 0, value: null }],
  enum_tuple: [choice, { tag: 1, value: "hi" }],
  enum_far: [choice, { tag: 300, value: 1n }],
  result_err: [result, { tag: 1, value: 7 }],
  record: [recordType, { id: 7, key: Buffer.of(1, 2, 3, 4), digests: [Buffer.alloc(4, 0xaa), Buffer.alloc(4, 0xbb)], label: "hi", choice: { tag: 300, value: 1n } }],
};

for (const vector of vectors.bare) {
  test(`dense/current typed vector: ${vector.name}`, () => {
    const [type, expected] = cases[vector.name];
    for (const layout of ["current", "dense"]) {
      codec.withNoritoWireLayout(layout, () => {
        const wire = Buffer.from(layout === "current" ? vector.compact : vector.dense, "hex");
        const decoded = type.decode(wire);
        assert.deepEqual(decoded, expected);
        assert.deepEqual(type.encode(decoded), wire);
        assert.throws(() => type.decode(Buffer.concat([wire, Buffer.of(0)])));
        assert.throws(() => type.decode(wire.subarray(0, -1)));
      });
    }
  });
}

for (const vector of vectors.frames) {
  test(`dense/current framed vector: ${vector.name}`, () => {
    const type = record([["value", raw(16)], ["tag", raw(2)]]);
    for (const layout of ["current", "dense"]) {
      const wire = Buffer.from(layout === "current" ? vector.compact : vector.dense, "hex");
      const frame = codec.validateNoritoFrame(wire, { expectedTypeName: vector.schema_name, expectedLayout: layout });
      assert.equal(frame.layout, layout);
      codec.withNoritoWireLayout(frame.layout, () => {
        const decoded = type.decode(frame.payload);
        assert.deepEqual(decoded, { value: Buffer.from("100f0e0d0c0b0a090807060504030201", "hex"), tag: Buffer.of(0xca, 0xfe) });
        assert.deepEqual(codec.frameNoritoPayload(type.encode(decoded), frame.schemaHash, frame.flags, layout === "current" ? 8 : 0), wire);
        assert.deepEqual(codec.decodeNoritoFrame(wire, context, frame.schemaHash), frame);
      }, frame.flags);
      assert.throws(() => codec.validateNoritoFrame(wire, { expectedLayout: layout === "current" ? "dense" : "current" }), /layout/);
    }
  });
}

test("dense frame rejects unknown minors, flags, padding, truncation, schema mismatch and suffixes", () => {
  const wire = Buffer.from(vectors.frames[0].dense, "hex");
  for (const [offset, value] of [[4, 1], [5, 2], [39, 2], [39, 0x80], [6, 0]]) {
    const bad = Buffer.from(wire); bad[offset] = value;
    assert.throws(() => codec.validateNoritoFrame(bad, { expectedTypeName: vectors.frames[0].schema_name }));
  }
  for (let end = 0; end < wire.length; end += 1) assert.throws(() => codec.validateNoritoFrame(wire.subarray(0, end)));
  assert.throws(() => codec.validateNoritoFrame(Buffer.concat([wire.subarray(0, 40), Buffer.of(0), wire.subarray(40)])), /padding/);
  assert.throws(() => codec.validateNoritoFrame(Buffer.concat([wire, Buffer.of(0)])), /trailing/);
  assert.throws(() => codec.validateNoritoFrame(wire, { expectedLayout: "unknown" }), /layout/);
  assert.throws(() => codec.decodeNoritoFrame(wire, context), /layout/);
  codec.withNoritoWireLayout("dense", () => {
    for (const [flags, padding] of [[2, 0], [0, 1]]) assert.throws(() => codec.frameNoritoPayload(Buffer.of(0), Buffer.alloc(16, 1), flags, padding));
  });
});

test("frame options are exact own data fields and never execute layout accessors", () => {
  const wire = Buffer.from(vectors.frames[0].dense, "hex");
  let getterCalls = 0;
  for (const options of [
    null, [], Object.create({ expectedLayout: "current" }),
    { layout: "current" }, { [Symbol("layout")]: "dense" },
    Object.defineProperty({}, "expectedLayout", { value: "dense" }),
    { get expectedLayout() { getterCalls += 1; return "dense"; } },
  ]) assert.throws(() => codec.validateNoritoFrame(wire, options), /options/);
  assert.equal(getterCalls, 0);
  assert.equal(codec.validateNoritoFrame(wire, Object.assign(Object.create(null), { expectedLayout: "dense" })).layout, "dense");
});

test("dense varints and count geometry fail closed, scalar u32 remains fixed", () => {
  codec.withNoritoWireLayout("dense", () => {
    for (const hex of ["8000", "800100", "80", "ffffffffffffffffff02", "ffffffffffffffffffff01"]) {
      assert.throws(() => codec.decodeNoritoVec(Buffer.from(hex, "hex"), () => { throw new Error("should never allocate"); }, context));
    }
    assert.throws(() => new codec.BufferReader(Buffer.from("8080808010", "hex"), context).readEnumTag("tag"), /u32/);
    for (const value of [-1, 0x1_0000_0000, 1.5]) assert.throws(() => codec.encodeNoritoEnumTag(value));
    for (const value of [-1, 1n << 64n]) assert.throws(() => codec.encodeUnsignedLeb128(value));
    assert.equal(codec.encodeU32Value(300, context).toString("hex"), "2c010000");
    assert.equal(codec.encodeNoritoEnumTag(300).toString("hex"), "ac02");
    assert.throws(() => new codec.BufferReader(Buffer.of(1), context).readBytes(-1, "bad"));
    assert.throws(() => codec.decodeNoritoByteArray(Buffer.of(1), -1, context));
  });
});

test("dense contexts nest and restore on failure; current fixed lengths stay fixed", () => {
  assert.equal(codec.getNoritoWireLayout(), "current");
  codec.withNoritoWireLayout("dense", () => {
    const reader = new codec.BufferReader(Buffer.of(1), context);
    assert.throws(() => codec.withNoritoWireLayout("current", () => { throw new Error("sentinel"); }), /sentinel/);
    assert.equal(codec.getNoritoWireLayout(), "dense");
    codec.withNoritoWireLayout("current", () => assert.equal(reader.readCount("count"), 1));
  });
  assert.equal(codec.getNoritoWireLayout(), "current");
  codec.withNoritoWireLayout("current", () => assert.equal(codec.encodeNoritoField(Buffer.of(7)).toString("hex"), "010000000000000007"), 0);
  assert.throws(() => codec.withNoritoWireLayout("future", () => {}));
  assert.throws(() => codec.withNoritoWireLayout("dense", () => {}, 2));
});

test("ConstVec bytes and nested EventFilterBox frames inherit the exact layout", () => {
  const schemaHash = codec.schemaHashForTypeName("iroha_data_model::events::model::EventFilterBox");
  for (const layout of ["current", "dense"]) {
    codec.withNoritoWireLayout(layout, () => {
      const bytes = Buffer.alloc(128, 0xa5);
      const wire = codec.encodeConstVecU8Value(bytes);
      assert.deepEqual(codec.decodeConstVecU8Value(wire, context), bytes);
      assert.equal(wire.length, layout === "dense" ? 130 : 264);
      assert.throws(() => codec.decodeConstVecU8Value(wire.subarray(0, -1), context));
      assert.throws(() => codec.decodeConstVecU8Value(Buffer.concat([wire, Buffer.of(0)]), context));
      const payload = codec.encodeEnumTagValue(0);
      const frame = codec.frameNoritoPayload(payload, schemaHash, layout === "dense" ? 0 : 2);
      assert.deepEqual(codec.encodeEventFilterBoxFramePayload(frame.toString("base64"), context), payload);
      assert.equal(codec.decodeEventFilterBoxFramePayload(payload, context), frame.toString("base64"));
      const wrongLayout = Buffer.from(frame); wrongLayout[5] = layout === "dense" ? 0 : 1;
      assert.throws(() => codec.encodeEventFilterBoxFramePayload(wrongLayout.toString("base64"), context));
    });
  }
});

test("current-only typed frame owners reject dense before payload interpretation", () => {
  const denseFrame = (schemaHash) => codec.frameNoritoPayload(Buffer.of(0), schemaHash, 0, 0, "dense");
  const blockProof = denseFrame(codec.schemaHashForTypeName("iroha_data_model::block::proofs::BlockProofs"));
  assert.equal(codec.validateNoritoFrame(blockProof).layout, "dense");
  assert.throws(() => noritoDecodeBlockProofs(blockProof), /layout dense does not match expected current/);
  const assessment = denseFrame(Buffer.from("72ca99de9d49cae6dfc07a2a628eb918", "hex"));
  assert.throws(() => decodeRetailFeeAssessmentMarkerMessage(`iroha:retail_fee:assessment:v1:${assessment.toString("hex")}`), /layout dense does not match expected current/);
  const race = denseFrame(codec.schemaHashForTypeName("iroha_data_model::classed_race_v1::ClassedRaceClassV1"));
  assert.throws(() => decodeClassedRaceFrameV1("ClassedRaceClassV1", race), /layout dense does not match expected current/);
});

test("Parliament state frames retain their declared current-layout contract", () => {
  const id = "ab".repeat(32);
  const response = {
    version: 1, current_height: 20,
    attempt: { id, proposal_content_id: "cd".repeat(32), sequence: 0, risk_tier: { tier: "Standard" }, stage: { stage: "Qualification" }, status: { status: "Active" } },
    policy_version: 1,
    required_bodies: [{ body: "rules-committee", decision_mode: { mode: "PublicFinding" } }],
    body_states: [{ body: "rules-committee", body_instance_id: null, status: null, public_finding_opened_at_height: null, public_finding_phase_blocks: null, public_finding_deadline_height: null, no_result_kind: null, no_result_height: null }],
    certificate: null, terminal_height: null, execution_failure_root: null, superseding_head: null,
  };
  const state = (layout) => codec.frameNoritoPayload(Buffer.of(0), Buffer.alloc(16, 1), 0, 0, layout).toString("hex");
  assert.equal(normalizeParliamentAttemptReadResponseV1({ ...response, state_payload_hex: state("current") }, id).state_payload_hex, state("current"));
  assert.throws(() => normalizeParliamentAttemptReadResponseV1({ ...response, state_payload_hex: state("dense") }, id), /layout dense does not match expected current/);
});


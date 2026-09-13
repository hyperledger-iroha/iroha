import assert from "node:assert/strict";
import test from "node:test";
import { _createNoritoInstructionApi } from "../src/norito.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";

const instruction = { Log: { level: "INFO", msg: "canonical owner fixture" } };
const json = JSON.stringify(instruction);

test("multisig instruction vector embeds exact Rust archives without rebuilding inner frames", () => {
  const second = { Log: { level: "WARN", msg: "second owner fixture" } };
  const archiveByJson = new Map([
    [json, Buffer.of(0xde, 0xad, 0xbe, 0xef)],
    [JSON.stringify(second), Buffer.of(0x91, 0x23, 0x45)],
  ]);
  const archiveCalls = [];
  const api = _createNoritoInstructionApi(createNativeRuntime({
    noritoEncodeInstruction: (value) => Buffer.from(value),
    noritoDecodeInstruction: (bytes) => Buffer.from(bytes).toString(),
    noritoEncodeInstructionBoxArchive(value) {
      archiveCalls.push(value);
      return archiveByJson.get(value);
    },
  }));
  const actual = api._encodeMultisigInstructions([instruction, second], 753);
  assert.deepEqual(actual, Buffer.from([
    2, 0, 0, 0, 0, 0, 0, 0, // vector count
    4, 0xde, 0xad, 0xbe, 0xef, // one length, exact first archive
    3, 0x91, 0x23, 0x45, // one length, exact second archive
  ]));
  assert.deepEqual(archiveCalls, [json, JSON.stringify(second)]);
  const rejection = new Error("owner rejected embedded instruction");
  const rejected = _createNoritoInstructionApi(createNativeRuntime({
    noritoEncodeInstruction: () => Buffer.of(1),
    noritoDecodeInstruction: () => json,
    noritoEncodeInstructionBoxArchive() { throw rejection; },
  }));
  assert.throws(() => rejected._encodeMultisigInstructions([instruction], 753), (error) => error === rejection);
});

test("all four instruction APIs use the canonical owner even for formerly pure JS variants", () => {
  const calls = [];
  const frame = Buffer.from([1, 2, 3]);
  const archive = Buffer.from([4, 5, 6]);
  const api = _createNoritoInstructionApi(createNativeRuntime({
    noritoEncodeInstruction(value) { calls.push(["encodeFrame", value]); return frame; },
    noritoDecodeInstruction(value) { calls.push(["decodeFrame", Buffer.from(value)]); return json; },
    noritoEncodeInstructionBoxArchive(value) { calls.push(["encodeArchive", value]); return archive; },
    noritoDecodeInstructionBoxArchive(value) { calls.push(["decodeArchive", Buffer.from(value)]); return json; },
  }));
  assert.deepEqual(api.noritoEncodeInstruction(instruction, 753), frame);
  assert.deepEqual(api.noritoDecodeInstruction(frame, 753), instruction);
  assert.deepEqual(api.noritoEncodeInstructionBoxArchive(instruction, 753), archive);
  assert.deepEqual(api.noritoDecodeInstructionBoxArchive(archive, 753), instruction);
  assert.deepEqual(new Set(calls.map(([name]) => name)), new Set(["encodeFrame", "decodeFrame", "encodeArchive", "decodeArchive"]));
  assert.equal(calls.find(([name]) => name === "encodeArchive")[1], json);
});

test("owner rejection never falls back to a local encoder or archive decoder", () => {
  const rejection = new Error("canonical owner rejected fixture");
  const fail = () => { throw rejection; };
  const api = _createNoritoInstructionApi(createNativeRuntime({
    noritoEncodeInstruction: fail, noritoDecodeInstruction: fail,
    noritoEncodeInstructionBoxArchive: fail, noritoDecodeInstructionBoxArchive: fail,
  }));
  for (const operation of [
    () => api.noritoEncodeInstruction(instruction, 753),
    () => api.noritoDecodeInstruction(Buffer.of(1), 753),
    () => api.noritoEncodeInstructionBoxArchive(instruction, 753),
    () => api.noritoDecodeInstructionBoxArchive(Buffer.of(1), 753),
  ]) assert.throws(operation, (error) => error === rejection);
});

test("preencoded instruction frames are checked by Rust instead of passed through", () => {
  const api = _createNoritoInstructionApi(createNativeRuntime({
    noritoDecodeInstruction: () => json,
    noritoEncodeInstruction: () => Buffer.of(1, 2),
  }));
  assert.throws(() => api.noritoEncodeInstruction(Buffer.of(1, 2, 3), 753), /not canonical/);
  assert.throws(() => api.noritoEncodeInstruction(Buffer.of(1, 2, 3).toString("base64"), 753), /not canonical/);
  assert.deepEqual(api.noritoEncodeInstruction(Buffer.of(1, 2), 753), Buffer.of(1, 2));
});

test("frame decode retains raw JSON mode and owner receives only the requested byte view", () => {
  const backing = Uint8Array.of(99, 1, 2, 99);
  const api = _createNoritoInstructionApi(createNativeRuntime({
    noritoDecodeInstruction(bytes) {
      assert.deepEqual(Buffer.from(bytes), Buffer.of(1, 2));
      return json;
    },
  }));
  assert.equal(api.noritoDecodeInstruction(backing.subarray(1, 3), 753, { parseJson: false }), json);
});

test("instruction JSON text reaches Rust without rounding unsafe u64 tokens", () => {
  const exact = ' {"Custom":{"payload":{"large":18446744073709551615}}} ';
  const api = _createNoritoInstructionApi(createNativeRuntime({
    noritoEncodeInstruction(value) { assert.equal(value, exact); return Buffer.of(1); },
    noritoDecodeInstruction() { return exact; },
  }));
  assert.deepEqual(api.noritoEncodeInstruction(exact, 753), Buffer.of(1));
  assert.equal(api.noritoDecodeInstruction(Buffer.of(1), 753, { parseJson: false }), exact);
});

test("duplicate keys in JSON text remain visible to the canonical owner's rejection", () => {
  const exact = '{"Custom":{"payload":{"same":1,"same":2}}}';
  const rejection = new Error("owner rejected duplicate key");
  const api = _createNoritoInstructionApi(createNativeRuntime({
    noritoEncodeInstruction(value) { assert.equal(value, exact); throw rejection; },
  }));
  assert.throws(() => api.noritoEncodeInstruction(exact, 753), (error) => error === rejection);
});

test("object inputs reject unsafe integers, non-finite numbers and unsupported bigint before native serialization", () => {
  let calls = 0;
  const api = _createNoritoInstructionApi(createNativeRuntime({
    noritoEncodeInstruction() { calls += 1; return Buffer.of(1); },
  }));
  for (const value of [Number.MAX_SAFE_INTEGER + 1, -(Number.MAX_SAFE_INTEGER + 1), NaN, Infinity, -Infinity, 1n]) {
    assert.throws(() => api.noritoEncodeInstruction({ Custom: { payload: { nested: [value] } } }, 753), /exact JSON text/);
  }
  assert.equal(calls, 0);
  api.noritoEncodeInstruction({ Custom: { payload: { finiteFraction: 1.25, safe: Number.MAX_SAFE_INTEGER } } }, 753);
  assert.equal(calls, 1);
});

test("parsed owner output rejects rounded integers while preserving fractional JSON and exact raw mode", () => {
  for (const token of ["9007199254740993", "-9007199254740993", "18446744073709551615", "1e999"]) {
    const exact = `{"Custom":{"payload":{"value":${token}}}}`;
    const api = _createNoritoInstructionApi(createNativeRuntime({
      noritoDecodeInstruction: () => exact,
      noritoDecodeInstructionBoxArchive: () => exact,
    }));
    assert.throws(() => api.noritoDecodeInstruction(Buffer.of(1), 753), /exact JSON text/);
    assert.throws(() => api.noritoDecodeInstructionBoxArchive(Buffer.of(1), 753), /exact JSON text/);
    assert.equal(api.noritoDecodeInstruction(Buffer.of(1), 753, { parseJson: false }), exact);
  }
  const exact = '{"Custom":{"payload":{"value":1.25,"safe":9007199254740991}}}';
  const api = _createNoritoInstructionApi(createNativeRuntime({
    noritoDecodeInstruction: () => exact,
    noritoDecodeInstructionBoxArchive: () => exact,
  }));
  assert.deepEqual(api.noritoDecodeInstruction(Buffer.of(1), 753), JSON.parse(exact));
  assert.deepEqual(api.noritoDecodeInstructionBoxArchive(Buffer.of(1), 753), JSON.parse(exact));
});

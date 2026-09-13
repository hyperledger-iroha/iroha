import assert from "node:assert/strict";
import test from "node:test";
import { _createNoritoInstructionApi } from "../src/norito.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";

// Injection exercises rejection only; successful instruction coding always uses
// the canonical native owner through the public adapter.
test("instruction adapter propagates native unavailability without an alternate codec", () => {
  const unavailable = new Error("native instruction codec unavailable");
  const calls = [];
  const api = _createNoritoInstructionApi(createNativeRuntime({
    noritoEncodeInstruction() {
      calls.push("encode");
      throw unavailable;
    },
    noritoDecodeInstruction() {
      calls.push("decode");
      throw unavailable;
    },
  }));
  const instruction = {
    Register: { Domain: { id: "wonderland", logo: null, metadata: {} } },
  };
  assert.throws(() => api.noritoEncodeInstruction(instruction), (error) => error === unavailable);
  assert.throws(() => api.noritoEncodeInstruction(JSON.stringify(instruction)), (error) => error === unavailable);
  assert.throws(() => api.noritoDecodeInstruction(Buffer.of(1)), (error) => error === unavailable);
  assert.deepEqual(calls, ["encode", "encode", "decode"]);
});

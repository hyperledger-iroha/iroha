import assert from "node:assert/strict";
import test from "node:test";
import { blake3 } from "@noble/hashes/blake3.js";
import { postDaIngestPhase } from "../src/dataAvailability.js";

const partSize = 1024 * 1024;

test("multipart DA resumes at the first missing part and finishes with signed metadata", async () => {
  const wire = Buffer.alloc(partSize + 3, 0x51);
  const request = { network_id: "network", lane_id: 0, epoch: 5, payload: wire.toString("base64"), signatures: ["witness"] };
  const calls = [];
  const result = await postDaIngestPhase(request, wire, async (path, body, status) => {
    assert.equal(path, "/v1/da/ingest/parts");
    assert.deepEqual(body.upload.wire_hash, [Array.from(blake3(wire))]);
    assert.equal(body.upload.wire_bytes, wire.length);
    const { kind, value } = body.action;
    calls.push(kind);
    if (kind === "Finish") {
      assert.equal(status, 202);
      assert.deepEqual(value, { ...request, payload: "" });
      return "submitted";
    }
    assert.equal(status, 200);
    if (kind === "Put") assert.deepEqual(value, { index: 1, bytes_b64: wire.subarray(partSize).toString("base64") });
    else assert.equal(value, null);
    return { next_part: kind === "Begin" ? 1 : 2 };
  });
  assert.equal(result, "submitted");
  assert.deepEqual(calls, ["Begin", "Put", "Finish"]);
});

test("multipart DA rejects impossible progress and leaves the signed payload intact", async () => {
  const wire = Buffer.alloc(partSize + 1);
  const request = { payload: wire.toString("base64") };
  for (const ack of [3, -1, 0.5]) {
    await assert.rejects(postDaIngestPhase(request, wire, async () => ({ next_part: ack })), /invalid DA upload progress/u);
  }
  await assert.rejects(postDaIngestPhase(request, wire, async () => ({ next_part: 0 })), /inconsistent DA part acknowledgment/u);
  assert.equal(request.payload, wire.toString("base64"));
  const direct = { payload: "YQ==" };
  await postDaIngestPhase(direct, Buffer.from("a"), async (path, body, status) => {
    assert.equal(path, "/v1/da/ingest");
    assert.equal(body, direct);
    assert.equal(status, 202);
  });
});

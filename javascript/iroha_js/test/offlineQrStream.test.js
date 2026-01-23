import assert from "node:assert/strict";
import test from "node:test";

import {
  OfflineQrStreamDecoder,
  OfflineQrStreamEncoder,
  OfflineQrStreamFrameKind,
} from "../src/offlineQrStream.js";

function buildPayload(size = 1200) {
  const buffer = Buffer.alloc(size);
  for (let i = 0; i < buffer.length; i += 1) {
    buffer[i] = (i * 31 + 7) % 256;
  }
  return buffer;
}

test("offline qr stream round-trips payloads", () => {
  const payload = buildPayload();
  const frames = OfflineQrStreamEncoder.encodeFrameBytes(payload, {
    chunkSize: 200,
  });
  const decoder = new OfflineQrStreamDecoder();
  let result = null;
  for (const frame of frames) {
    result = decoder.ingest(frame);
  }
  assert.ok(result);
  assert.equal(result.isComplete, true);
  assert.deepEqual(result.payload, payload);
});

test("offline qr stream recovers a missing frame with parity", () => {
  const payload = buildPayload(900);
  const frames = OfflineQrStreamEncoder.encodeFrames(payload, {
    chunkSize: 180,
    parityGroup: 3,
  });
  const header = frames.find((frame) => frame.kind === OfflineQrStreamFrameKind.header);
  const dataFrames = frames.filter((frame) => frame.kind === OfflineQrStreamFrameKind.data);
  const parityFrames = frames.filter((frame) => frame.kind === OfflineQrStreamFrameKind.parity);
  assert.ok(header);
  assert.equal(dataFrames.length > 0, true);
  assert.equal(parityFrames.length > 0, true);
  const dropped = dataFrames[1];
  const decoder = new OfflineQrStreamDecoder();
  decoder.ingest(header.encode());
  let result = null;
  for (const frame of dataFrames) {
    if (frame !== dropped) {
      result = decoder.ingest(frame.encode());
    }
  }
  for (const frame of parityFrames) {
    result = decoder.ingest(frame.encode());
  }
  assert.ok(result);
  assert.equal(result.isComplete, true);
  assert.deepEqual(result.payload, payload);
});

test("offline qr stream rejects frames with invalid checksum", () => {
  const payload = buildPayload(400);
  const frames = OfflineQrStreamEncoder.encodeFrameBytes(payload);
  const frame = Buffer.from(frames[0]);
  frame[frame.length - 1] ^= 0x22;
  const decoder = new OfflineQrStreamDecoder();
  assert.throws(() => decoder.ingest(frame), /checksum/i);
});

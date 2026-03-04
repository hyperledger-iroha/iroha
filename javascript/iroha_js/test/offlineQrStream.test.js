import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

import {
  OfflineQrStreamDecoder,
  OfflineQrStreamEncoder,
  OfflineQrStreamFrameKind,
  OfflineQrStreamFrameEncoding,
  OfflineQrStreamScanSession,
  encodeQrFrameText,
  decodeQrFrameText,
  scanQrStreamFrames,
  sakuraStormQrStreamTheme,
  sakuraStormQrStreamSkin,
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

test("offline qr stream text codec round-trips", () => {
  const payload = buildPayload(128);
  const encoded = encodeQrFrameText(payload, OfflineQrStreamFrameEncoding.base64);
  const decoded = decodeQrFrameText(encoded, OfflineQrStreamFrameEncoding.base64);
  assert.deepEqual(decoded, payload);
});

test("offline qr stream scan loop ingests frames", async () => {
  const payload = buildPayload(512);
  const frames = OfflineQrStreamEncoder.encodeFrameBytes(payload, { chunkSize: 200 });
  const encodedFrames = frames.map((frame) =>
    encodeQrFrameText(frame, OfflineQrStreamFrameEncoding.base64),
  );
  const result = await scanQrStreamFrames(encodedFrames, {
    frameEncoding: OfflineQrStreamFrameEncoding.base64,
  });
  assert.ok(result);
  assert.equal(result.isComplete, true);
  assert.deepEqual(result.payload, payload);
});

test("offline qr stream scan session ingests text frames", () => {
  const payload = buildPayload(256);
  const frames = OfflineQrStreamEncoder.encodeFrameBytes(payload, { chunkSize: 200 });
  const session = new OfflineQrStreamScanSession({
    frameEncoding: OfflineQrStreamFrameEncoding.base64,
  });
  let result = null;
  for (const frame of frames) {
    const encoded = encodeQrFrameText(frame, OfflineQrStreamFrameEncoding.base64);
    result = session.ingest(encoded);
  }
  assert.ok(result);
  assert.equal(result.isComplete, true);
  assert.deepEqual(result.payload, payload);
});

test("offline qr stream fixtures round-trip", () => {
  const fixturePath = new URL("../../../fixtures/qr_stream/qr_stream_basic.json", import.meta.url);
  const fixture = JSON.parse(fs.readFileSync(fixturePath, "utf8"));
  const payload = Buffer.from(fixture.payload_hex, "hex");
  const frames = fixture.frames.map((frame) => Buffer.from(frame.bytes_hex, "hex"));
  const decoder = new OfflineQrStreamDecoder();
  let result = null;
  for (const frame of frames) {
    result = decoder.ingest(frame);
  }
  assert.ok(result);
  assert.equal(result.isComplete, true);
  assert.deepEqual(result.payload, payload);
});

test("offline qr stream sakura-storm skin matches preset", () => {
  assert.equal(sakuraStormQrStreamTheme.name, "sakura-storm");
  assert.deepEqual(sakuraStormQrStreamTheme.backgroundStart, {
    red: 0.05,
    green: 0.02,
    blue: 0.08,
  });
  assert.equal(sakuraStormQrStreamSkin.name, "sakura-storm");
  assert.equal(sakuraStormQrStreamSkin.frameRate, 12);
  assert.equal(sakuraStormQrStreamSkin.petalDriftSpeed, 0.6);
  assert.equal(sakuraStormQrStreamSkin.progressOverlayAlpha, 0.34);
});

test("offline qr stream sakura-storm preset recovers dropped frame in scan loop", () => {
  const payload = buildPayload(6 * 1024);
  const frames = OfflineQrStreamEncoder.encodeFrames(payload, {
    chunkSize: 336,
    parityGroup: 4,
  });
  const header = frames.find((frame) => frame.kind === OfflineQrStreamFrameKind.header);
  const dataFrames = frames.filter((frame) => frame.kind === OfflineQrStreamFrameKind.data);
  const parityFrames = frames.filter((frame) => frame.kind === OfflineQrStreamFrameKind.parity);
  assert.ok(header);
  assert.equal(dataFrames.length > 0, true);
  assert.equal(parityFrames.length > 0, true);

  const session = new OfflineQrStreamScanSession({
    frameEncoding: OfflineQrStreamFrameEncoding.base64,
  });
  session.ingest(encodeQrFrameText(header.encode(), OfflineQrStreamFrameEncoding.base64));
  let result = null;
  for (const frame of dataFrames) {
    if (frame.index === 1) {
      continue;
    }
    result = session.ingest(
      encodeQrFrameText(frame.encode(), OfflineQrStreamFrameEncoding.base64),
    );
  }
  for (const frame of parityFrames) {
    result = session.ingest(
      encodeQrFrameText(frame.encode(), OfflineQrStreamFrameEncoding.base64),
    );
  }
  assert.ok(result);
  assert.equal(result.isComplete, true);
  assert.deepEqual(result.payload, payload);
  assert.equal(result.recoveredChunks, 1);
});

import assert from "node:assert/strict";
import test from "node:test";

import {
  OfflinePetalStreamEncoder,
  OfflinePetalStreamDecoder,
  OfflinePetalStreamOptions,
  OfflinePetalStreamScanSession,
  samplePetalStreamGridFromRgba,
  decodePetalStreamFrameAuto,
} from "../src/offlinePetalStream.js";
import { OfflineQrStreamEncoder } from "../src/offlineQrStream.js";

function buildPayload(size = 256) {
  const buffer = Buffer.alloc(size);
  for (let i = 0; i < buffer.length; i += 1) {
    buffer[i] = (i * 17 + 13) % 256;
  }
  return buffer;
}

function renderGridToRgba(grid, cellSize = 4) {
  const size = grid.gridSize * cellSize;
  const data = Buffer.alloc(size * size * 4);
  for (let y = 0; y < grid.gridSize; y += 1) {
    for (let x = 0; x < grid.gridSize; x += 1) {
      const idx = y * grid.gridSize + x;
      const value = grid.cells[idx] ? 0 : 255;
      for (let oy = 0; oy < cellSize; oy += 1) {
        for (let ox = 0; ox < cellSize; ox += 1) {
          const px = x * cellSize + ox;
          const py = y * cellSize + oy;
          const offset = (py * size + px) * 4;
          data[offset] = value;
          data[offset + 1] = value;
          data[offset + 2] = value;
          data[offset + 3] = 255;
        }
      }
    }
  }
  return { data, width: size, height: size };
}

test("offline petal stream grid round-trips payloads", () => {
  const payload = buildPayload(120);
  const grid = OfflinePetalStreamEncoder.encodeGrid(payload, new OfflinePetalStreamOptions());
  const decoded = OfflinePetalStreamDecoder.decodeGrid(grid, new OfflinePetalStreamOptions());
  assert.deepEqual(decoded, payload);
});

test("offline petal stream samples decode from rgba", () => {
  const payload = buildPayload(180);
  const grid = OfflinePetalStreamEncoder.encodeGrid(payload, new OfflinePetalStreamOptions());
  const image = renderGridToRgba(grid, 5);
  const samples = samplePetalStreamGridFromRgba(image, grid.gridSize);
  const decoded = OfflinePetalStreamDecoder.decodeSamples(samples, new OfflinePetalStreamOptions());
  assert.deepEqual(decoded, payload);
});

test("offline petal stream auto detects grid size", () => {
  const payload = buildPayload(140);
  const grid = OfflinePetalStreamEncoder.encodeGrid(payload, new OfflinePetalStreamOptions());
  const image = renderGridToRgba(grid, 6);
  const result = decodePetalStreamFrameAuto(image, new OfflinePetalStreamOptions());
  assert.equal(result.gridSize, grid.gridSize);
  assert.deepEqual(result.payload, payload);
});

test("offline petal stream scan session assembles qr payload", () => {
  const payload = buildPayload(420);
  const frames = OfflineQrStreamEncoder.encodeFrameBytes(payload, { chunkSize: 180 });
  const session = new OfflinePetalStreamScanSession();
  let result = null;
  for (const frameBytes of frames) {
    const grid = OfflinePetalStreamEncoder.encodeGrid(frameBytes, new OfflinePetalStreamOptions());
    const image = renderGridToRgba(grid, 4);
    const samples = samplePetalStreamGridFromRgba(image, grid.gridSize);
    result = session.ingestSampleGrid(samples);
  }
  assert.ok(result);
  assert.equal(result.isComplete, true);
  assert.deepEqual(result.payload, payload);
});

test("offline petal stream encodeGrids keeps a consistent grid size", () => {
  const small = buildPayload(12);
  const large = buildPayload(240);
  const result = OfflinePetalStreamEncoder.encodeGrids([small, large]);
  assert.equal(result.grids.length, 2);
  assert.equal(result.gridSize, result.grids[0].gridSize);
  assert.equal(result.gridSize, result.grids[1].gridSize);
});

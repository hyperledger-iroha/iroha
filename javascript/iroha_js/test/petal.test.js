import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { inflateSync } from "node:zlib";

import { build } from "esbuild";
import ts from "typescript";

import { findForbiddenBrowserInputs } from "../scripts/bundle-size-check.mjs";
import {
  PETAL_DEFAULT_DECODE_OPTIONS,
  PETAL_DEFAULT_SCAN_LIMITS,
  PETAL_GLYPHS,
  PETAL_LANES,
  PETAL_LAYOUT,
  PETAL_PALETTE,
  PETAL_STREAM,
  PetalCameraScanner,
  PetalError,
  PetalFountainDecoder,
  PetalFrameCells,
  PetalHomography,
  PetalLuma,
  PetalReedSolomon,
  PetalScanSession,
  PetalStreamAssembler,
  PetalStreamEncoder,
  PetalStreamPlayer,
  PetalXorshift32,
  atomsInFrame,
  blossoms,
  crc32c,
  dataSlots,
  decodeLane,
  decodeLaneCounted,
  decodePetalFrame,
  decodePetalFrameAt,
  drawPetalFrame,
  encodeAtom,
  encodeLane,
  finderLit,
  firstAtomId,
  generateTemplates,
  isBeaconFrame,
  labelComponents,
  laneWhitening,
  locate,
  maskWords,
  mix32,
  observedCells,
  parseAtomLane,
  parseDLane,
  petalDrawList,
  petalPlayerFrame,
  petalScanSize,
  renderPetalFrame,
  selectQuad,
  slotCenter,
  slotRoles,
  splitPayload,
  splitSlot,
  tileCenter,
  tileMatchError,
} from "../src/petal.js";
import {
  decodeWithErasures,
  patchLevels,
  readTileLanes,
  readTiles,
  readTilesNormalised,
  referenceLevels,
  rescale,
  samplePatches,
  tileWords,
} from "../src/petal/decode.js";

const STREAM_FIXTURE = new URL("../../../fixtures/petal/petal_stream_v1.json", import.meta.url);
const CAPTURE_FIXTURE = new URL("../../../fixtures/petal/petal_captures_v1.json", import.meta.url);

const hex = (bytes) => Buffer.from(bytes).toString("hex");
const fromHex = (text) => Uint8Array.from(Buffer.from(text, "hex"));

/** Deterministic payload from the reference generator. */
function payload(len, seed) {
  const rng = new PetalXorshift32(seed);
  const out = new Uint8Array(len);
  for (let index = 0; index < len; index += 1) out[index] = rng.nextByte();
  return out;
}

/** The reference tests' 64-bit LCG. */
class Lcg {
  constructor(seed) {
    this.state = BigInt(seed);
  }

  next() {
    this.state = (this.state * 6364136223846793005n + 1442695040888963407n) & 0xffffffffffffffffn;
    return Number(this.state >> 33n);
  }

  byte() {
    return this.next() & 0xff;
  }

  below(bound) {
    return this.next() % bound;
  }
}

function assertPetalError(fn, code) {
  assert.throws(fn, (error) => error instanceof PetalError && error.code === code);
}

function renderLuma(cells, size, supersample) {
  const image = renderPetalFrame(cells, { size, supersample });
  return PetalLuma.fromRgba(image.width, image.height, image.data);
}

function feedFrame(assembler, encoder, frame, lanes) {
  const words = encoder.words(frame);
  for (const lane of lanes) {
    const data = decodeLane(lane, words[lane.toLowerCase()]);
    if (lane === "D") {
      assembler.pushDLane(parseDLane(data));
    } else {
      assembler.pushAtoms(parseAtomLane(lane, data));
    }
  }
}

function reassemble(atoms, len) {
  const out = new Uint8Array(atoms.length * 16);
  atoms.forEach((atom, index) => out.set(atom, index * 16));
  return out.slice(0, len);
}

// ---------------------------------------------------------------- CRC / PRNG

test("crc32c matches the published check value and is zero for empty input", () => {
  assert.equal(crc32c(new TextEncoder().encode("123456789")), 0xe3069283);
  assert.equal(crc32c(new Uint8Array()), 0);
});

test("xorshift32 matches the reference sequence and remaps the zero seed", () => {
  const rng = new PetalXorshift32(1);
  assert.deepEqual([rng.nextU32(), rng.nextU32(), rng.nextU32()], [270369, 67634689, 2647435461]);
  const zero = new PetalXorshift32(0);
  const deadbeef = new PetalXorshift32(0xdeadbeef);
  for (let index = 0; index < 8; index += 1) assert.equal(zero.nextU32(), deadbeef.nextU32());
});

// ---------------------------------------------------------------- Reed-Solomon

test("Reed-Solomon matches the QR HELLO WORLD check vector", () => {
  const data = [32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17];
  const word = new PetalReedSolomon(10).encode(data);
  assert.deepEqual(Array.from(word.subarray(0, 16)), data);
  assert.deepEqual(Array.from(word.subarray(16)), [196, 35, 39, 119, 235, 215, 231, 226, 93, 23]);
});

test("Reed-Solomon corrects random errors and erasures up to capacity", () => {
  const rng = new Lcg(7);
  for (const [k, nsym] of [
    [16, 16],
    [60, 68],
    [12, 18],
    [13, 115],
  ]) {
    const rs = new PetalReedSolomon(nsym);
    for (let trial = 0; trial < 60; trial += 1) {
      const data = Uint8Array.from({ length: k }, () => rng.byte());
      const clean = rs.encode(data);
      const n = clean.length;
      const f = rng.below(Math.min(nsym, n - 1) + 1);
      const e = rng.below(Math.floor((nsym - f) / 2) + 1);
      const word = clean.slice();
      const positions = Array.from({ length: n }, (_, index) => index);
      for (let i = 0; i < f + e; i += 1) {
        const j = i + rng.below(n - i);
        [positions[i], positions[j]] = [positions[j], positions[i]];
      }
      const erased = positions.slice(0, f);
      for (const position of erased) word[position] = rng.byte();
      for (const position of positions.slice(f, f + e)) word[position] ^= rng.byte() | 1;
      const corrected = rs.decode(word, erased);
      assert.ok(corrected <= f + e);
      assert.deepEqual(word, clean, `k=${k} nsym=${nsym} f=${f} e=${e}`);
    }
  }
});

test("Reed-Solomon rejects words beyond capacity without returning wrong data", () => {
  const rng = new Lcg(99);
  const rs = new PetalReedSolomon(16);
  let wrongAccepts = 0;
  for (let trial = 0; trial < 200; trial += 1) {
    const data = Uint8Array.from({ length: 16 }, () => rng.byte());
    const clean = rs.encode(data);
    const word = clean.slice();
    const positions = Array.from({ length: word.length }, (_, index) => index);
    for (let i = 0; i < 20; i += 1) {
      const j = i + rng.below(word.length - i);
      [positions[i], positions[j]] = [positions[j], positions[i]];
    }
    for (const position of positions.slice(0, 20)) word[position] ^= rng.byte() | 1;
    let accepted = true;
    try {
      rs.decode(word, []);
    } catch (error) {
      assert.equal(error.code, "rs_uncorrectable");
      accepted = false;
    }
    if (accepted) {
      // a miscorrection must at least be a valid codeword
      assert.deepEqual(rs.encode(word.subarray(0, 16)), word);
      if (hex(word) !== hex(clean)) wrongAccepts += 1;
    }
  }
  assert.ok(wrongAccepts <= 2, `miscorrection rate too high: ${wrongAccepts}`);
});

test("Reed-Solomon rejects malformed arguments", () => {
  const rs = new PetalReedSolomon(4);
  assertPetalError(() => rs.decode(new Uint8Array(4), []), "rs_invalid_shape");
  const word = rs.encode([1, 2, 3]);
  assertPetalError(() => rs.decode(word, [9]), "rs_invalid_shape");
  assertPetalError(() => rs.decode(word, [1, 1]), "rs_invalid_shape");
  assert.throws(() => new PetalReedSolomon(0), RangeError);
  assert.throws(() => new PetalReedSolomon(255), RangeError);
  assert.throws(() => new PetalReedSolomon(10).encode(new Uint8Array(246)), RangeError);
});

// ---------------------------------------------------------------- layout and glyphs

test("the mask has 256 mirror-symmetric tiles that show top from bottom", () => {
  assert.equal(PETAL_LAYOUT.tiles.length, 256);
  for (const row of PETAL_LAYOUT.mask) {
    assert.equal(row.length, 20);
    for (let col = 0; col < 10; col += 1) assert.equal(row[col], row[19 - col], `row ${row} not mirrored`);
  }
  assert.notEqual(PETAL_LAYOUT.mask[0], PETAL_LAYOUT.mask[19]);
});

test("tiles are row-major and inside the canvas", () => {
  let previous = null;
  PETAL_LAYOUT.tiles.forEach(([col, row], index) => {
    if (previous !== null) {
      assert.ok(row > previous[1] || (row === previous[1] && col > previous[0]));
    }
    previous = [col, row];
    const [x, y] = tileCenter(index);
    assert.ok(x >= 0 && x < 1024 && y >= 0 && y < 1024);
  });
  assert.throws(() => tileCenter(256), RangeError);
});

test("ring slots provide exactly the lane D capacity", () => {
  const roles = slotRoles();
  assert.equal(roles.length, 276);
  assert.equal(roles.filter((role) => role.kind === "data").length, 240);
  assert.equal(roles.filter((role) => role.kind === "gate").length, 4 + 5 + 7);
  assert.equal(dataSlots().length, 240);
  // the two spare slots are the last non-reserved slots of the outer ring
  assert.equal(roles.filter((role) => role.kind === "spare").length, 2);
  assert.deepEqual(splitSlot(0), [0, 0]);
  assert.deepEqual(splitSlot(80), [1, 0]);
  assert.deepEqual(splitSlot(275), [2, 103]);
});

test("gates and guards never touch the top of a ring", () => {
  const roles = slotRoles();
  for (let flat = 0; flat < roles.length; flat += 1) {
    if (roles[flat].kind !== "gate" && roles[flat].kind !== "guard") continue;
    const [ring, slot] = splitSlot(flat);
    const top = (3 * PETAL_LAYOUT.ringSlots[ring]) / 4;
    assert.ok(Math.abs(slot - top) > 2, `ring ${ring} slot ${slot} near the top`);
  }
});

test("finders and rings do not overlap and tiles stay inside the inner ring", () => {
  const outermost = PETAL_LAYOUT.ringRadii[2] + PETAL_LAYOUT.dotRadius;
  for (const [x, y] of PETAL_LAYOUT.finderCenters) {
    const distance = Math.hypot(x - 512, y - 512);
    assert.ok(distance - PETAL_LAYOUT.finderOuter > outermost + 20);
  }
  let farthest = 0;
  for (let tile = 0; tile < 256; tile += 1) {
    const [x, y] = tileCenter(tile);
    const h = PETAL_LAYOUT.tileSize / 2;
    for (const [cx, cy] of [
      [x - h, y - h],
      [x + h, y - h],
      [x - h, y + h],
      [x + h, y + h],
    ]) {
      farthest = Math.max(farthest, Math.hypot(cx - 512, cy - 512));
    }
  }
  assert.ok(farthest + 10 < PETAL_LAYOUT.ringRadii[0] - PETAL_LAYOUT.dotRadius);
});

test("slot centres are the single-precision reference values", () => {
  assert.deepEqual(slotCenter(0, 0), [872, 512]);
  // f32 bit patterns of slot (0, 1) from the reference: 0x4459B8FA, 0x44070FB3
  const view = new DataView(new ArrayBuffer(4));
  const [x, y] = slotCenter(0, 1);
  view.setFloat32(0, x);
  assert.equal(view.getUint32(0), 0x4459b8fa);
  view.setFloat32(0, y);
  assert.equal(view.getUint32(0), 0x44070fb3);
  assert.ok(finderLit(0, 0) && finderLit(0, -34) && !finderLit(0, -59) && !finderLit(59, 59));
});

test("checked-in glyph templates match the stroke definitions", () => {
  const generated = generateTemplates();
  assert.equal(generated.length, 16);
  generated.forEach((template, glyph) => assert.deepEqual(Array.from(template), PETAL_GLYPHS.templates[glyph]));
});

test("every glyph has enough ink inside the design grid", () => {
  PETAL_GLYPHS.strokes.forEach((strokes, glyph) => {
    const total = PETAL_GLYPHS.templates[glyph].reduce((sum, value) => sum + value, 0);
    assert.ok(total > 255 * 6, `glyph ${glyph} has too little ink`);
    for (const stroke of strokes) {
      for (const value of stroke) assert.ok(value >= 2 && value <= 30);
    }
  });
});

test("glyphs are pairwise distinct under blur", () => {
  const feature = (glyph) => {
    const values = PETAL_GLYPHS.templates[glyph];
    const mean = values.reduce((sum, value) => sum + value, 0) / values.length;
    const centered = values.map((value) => value - mean);
    const norm = Math.sqrt(centered.reduce((sum, value) => sum + value * value, 0));
    return centered.map((value) => value / norm);
  };
  for (let a = 0; a < 16; a += 1) {
    for (let b = a + 1; b < 16; b += 1) {
      const fa = feature(a);
      const fb = feature(b);
      const dot = fa.reduce((sum, value, index) => sum + value * fb[index], 0);
      assert.ok(1 - dot > 0.2, `glyphs ${a} and ${b} are too similar: ${1 - dot}`);
    }
  }
});

// ---------------------------------------------------------------- lanes

test("lane sizes are consistent", () => {
  assert.deepEqual(
    [PETAL_LANES.P.wordLen, PETAL_LANES.K.wordLen, PETAL_LANES.D.wordLen],
    [32, 128, 30],
  );
  assert.deepEqual([PETAL_LANES.P.dataLen, PETAL_LANES.K.dataLen, PETAL_LANES.D.dataLen], [19, 83, 19]);
  for (const lane of ["P", "K", "D"]) {
    assert.equal(PETAL_LANES[lane].dataLen, PETAL_STREAM.laneHeaderLen + PETAL_LANES[lane].atoms * PETAL_STREAM.atomLen);
  }
});

test("whitening is deterministic and balanced", () => {
  for (const lane of ["P", "D", "K"]) {
    const a = laneWhitening(lane);
    assert.deepEqual(a, laneWhitening(lane));
    let ones = 0;
    for (const byte of a) for (let bit = 0; bit < 8; bit += 1) ones += (byte >> bit) & 1;
    const bits = a.length * 8;
    assert.ok(ones > Math.floor((bits * 38) / 100) && ones < Math.floor((bits * 62) / 100), `${lane} ones ${ones}/${bits}`);
  }
});

test("lanes roundtrip through cells", () => {
  const pData = Uint8Array.from({ length: 19 }, (_, b) => b);
  const kData = Uint8Array.from({ length: 83 }, (_, b) => (b * 37) & 0xff);
  const dData = Uint8Array.from({ length: 19 }, (_, b) => b ^ 0xa5);
  const p = encodeLane("P", pData);
  const k = encodeLane("K", kData);
  const d = encodeLane("D", dData);
  const cells = PetalFrameCells.fromWords(p, k, d);
  assert.deepEqual(cells.pWord(), p);
  assert.deepEqual(cells.kWord(), k);
  assert.deepEqual(cells.dWord(), d);
  assert.deepEqual(decodeLane("P", cells.pWord()), pData);
  assert.deepEqual(decodeLane("K", cells.kWord()), kData);
  assert.deepEqual(decodeLane("D", cells.dWord()), dData);
  assert.throws(() => encodeLane("P", new Uint8Array(18)), RangeError);
  assert.throws(() => encodeLane("X", pData), TypeError);
  assertPetalError(() => decodeLane("P", new Uint8Array(31)), "rs_invalid_shape");
});

test("all-zero data still lights roughly half the tiles", () => {
  const cells = PetalFrameCells.fromWords(
    encodeLane("P", new Uint8Array(19)),
    encodeLane("K", new Uint8Array(83)),
    encodeLane("D", new Uint8Array(19)),
  );
  const lit = cells.light.reduce((sum, value) => sum + value, 0);
  assert.ok(lit >= 90 && lit <= 166, `${lit} light tiles`);
});

test("gate dots are always lit and guards dark", () => {
  const cells = PetalFrameCells.fromWords(new Uint8Array(32).fill(0xff), new Uint8Array(128).fill(0xff), new Uint8Array(30).fill(0xff));
  slotRoles().forEach((role, slot) => {
    if (role.kind === "gate" || role.kind === "data") assert.equal(cells.dots[slot], 1);
    else assert.equal(cells.dots[slot], 0);
  });
});

test("counted decoding reports the rewritten positions", () => {
  const data = Uint8Array.from({ length: 19 }, (_, b) => b);
  const clean = encodeLane("P", data);
  const exact = decodeLaneCounted("P", clean);
  assert.deepEqual([Array.from(exact.data), exact.corrected], [Array.from(data), 0]);
  const damaged = clean.slice();
  for (const position of [0, 7, 19, 31]) damaged[position] ^= 0xc3;
  const counted = decodeLaneCounted("P", damaged, []);
  assert.deepEqual([Array.from(counted.data), counted.corrected], [Array.from(data), 4]);
  assert.deepEqual(decodeLane("P", damaged), data);
  assertPetalError(() => decodeLaneCounted("P", new Uint8Array(32).fill(0x55), []), "rs_uncorrectable");
});

test("lane decoding survives burst damage", () => {
  const kData = Uint8Array.from({ length: 83 }, (_, b) => b);
  const word = encodeLane("K", kData);
  for (let index = 0; index < 22; index += 1) word[index] ^= 0x5a;
  assert.deepEqual(decodeLane("K", word), kData);
});

// ---------------------------------------------------------------- fountain

test("systematic atoms alone recover the payload", () => {
  const data = payload(100, 5);
  const source = splitPayload(data);
  const decoder = new PetalFountainDecoder(source.length);
  source.forEach((atom, id) => assert.ok(decoder.addEncoded(7, id, atom)));
  assert.deepEqual(reassemble(decoder.solve(), 100), data);
});

test("repair atoms cover for lost systematic atoms", () => {
  const data = payload(1000, 9);
  const source = splitPayload(data);
  const k = source.length;
  const crc = 0x12345678;
  const decoder = new PetalFountainDecoder(k);
  for (let id = 0; id < k; id += 1) {
    if (id % 3 !== 0) decoder.addEncoded(crc, id, encodeAtom(source, crc, id));
  }
  let id = k;
  let used = 0;
  while (!decoder.isComplete()) {
    decoder.addEncoded(crc, id, encodeAtom(source, crc, id));
    id += 1;
    used += 1;
    assert.ok(used < k, "decoder must converge");
  }
  const missing = Array.from({ length: k }, (_, i) => i).filter((i) => i % 3 === 0).length;
  assert.ok(used <= missing + 8, `needed ${used} repairs for ${missing} missing`);
  assert.deepEqual(reassemble(decoder.solve(), 1000), data);
});

test("pure repair streams decode with small overhead", () => {
  const data = payload(5000, 11);
  const source = splitPayload(data);
  const k = source.length;
  const crc = 0xcafef00d;
  let totalOverhead = 0;
  for (let trial = 0; trial < 20; trial += 1) {
    const decoder = new PetalFountainDecoder(k);
    let id = k + trial * 1000;
    let received = 0;
    while (!decoder.isComplete()) {
      decoder.addEncoded(crc, id, encodeAtom(source, crc, id));
      id += 1;
      received += 1;
    }
    totalOverhead += received - k;
    assert.deepEqual(reassemble(decoder.solve(), 5000), data);
  }
  assert.ok(totalOverhead <= 20 * 4, `average overhead ${totalOverhead / 20}`);
});

test("duplicate and dependent atoms do not raise the rank", () => {
  const source = splitPayload(payload(60, 3));
  const decoder = new PetalFountainDecoder(source.length);
  assert.ok(decoder.addEncoded(1, 0, source[0]));
  assert.ok(!decoder.addEncoded(1, 0, source[0]));
  assert.equal(decoder.rank, 1);
  assert.equal(decoder.solve(), null);
});

test("repair masks span far more than thirty-two dimensions", () => {
  const k = 200;
  const decoder = new PetalFountainDecoder(k);
  for (let id = k; id < k + 400; id += 1) decoder.add(maskWords(k, 5, id), new Uint8Array(16));
  assert.equal(decoder.rank, k, "repair masks must reach full rank");
});

test("masks are nonzero and padded bits are clear", () => {
  for (const k of [1, 2, 31, 32, 33, 100]) {
    for (let id = 0; id < 200; id += 1) {
      const mask = maskWords(k, 99, id);
      assert.ok(mask.some((word) => word !== 0));
      if (k % 32 !== 0) assert.equal(mask[mask.length - 1] >>> (k % 32), 0);
    }
  }
});

test("fountain decoding tolerates loss and arbitrary reordering", () => {
  const data = payload(2300, 41);
  const source = splitPayload(data);
  const k = source.length;
  const crc = crc32c(data);
  // every id up to 3k, a third of them lost, the rest shuffled
  const rng = new PetalXorshift32(4242);
  const ids = [];
  for (let id = 0; id < 3 * k; id += 1) {
    if (rng.nextU32() % 3 !== 0) ids.push(id);
  }
  for (let index = ids.length - 1; index > 0; index -= 1) {
    const swap = rng.nextU32() % (index + 1);
    [ids[index], ids[swap]] = [ids[swap], ids[index]];
  }
  const decoder = new PetalFountainDecoder(k);
  let consumed = 0;
  for (const id of ids) {
    decoder.addEncoded(crc, id, encodeAtom(source, crc, id));
    consumed += 1;
    if (decoder.isComplete()) break;
  }
  assert.ok(decoder.isComplete());
  assert.ok(consumed <= k + 12, `consumed ${consumed} atoms for ${k} sources`);
  assert.deepEqual(reassemble(decoder.solve(), data.length), data);
});

// ---------------------------------------------------------------- stream

test("atom ids are contiguous across frames", () => {
  let expected = 0;
  for (let frame = 0; frame <= 70; frame += 1) {
    assert.equal(firstAtomId(frame), expected, `frame ${frame}`);
    expected += atomsInFrame(frame);
  }
  assert.equal(atomsInFrame(0), 6);
  assert.equal(atomsInFrame(1), 7);
  const encoder = new PetalStreamEncoder(payload(300, 1), 1);
  assert.equal(parseAtomLane("K", encoder.laneData(4).k).firstId, firstAtomId(4) + 1);
  assert.equal(parseAtomLane("K", encoder.laneData(5).k).firstId, firstAtomId(5) + 2);
  assert.equal(parseDLane(encoder.laneData(5).d).packet.firstId, firstAtomId(5) + 1);
  // the frame counter wraps on a beacon frame, so ids repeat cleanly
  assert.ok(isBeaconFrame(0) && 65536 % PETAL_STREAM.beaconInterval === 0);
});

test("a clean stream completes after one systematic pass", () => {
  const data = payload(1000, 21);
  const encoder = new PetalStreamEncoder(data, 2);
  const assembler = new PetalStreamAssembler();
  for (let frame = 0; frame < encoder.systematicFrames(); frame += 1) feedFrame(assembler, encoder, frame, ["D", "P", "K"]);
  const done = assembler.takeCompleted();
  assert.deepEqual(done.payload, data);
  assert.equal(done.meta.kind, 2);
  assert.ok(assembler.progress().complete);
});

test("any single lane is enough given a beacon", () => {
  const data = payload(400, 22);
  const encoder = new PetalStreamEncoder(data, 1);
  for (const lane of ["P", "K", "D"]) {
    const assembler = new PetalStreamAssembler();
    feedFrame(assembler, encoder, 0, ["D"]);
    for (let frame = 0; frame < 400; frame += 1) {
      feedFrame(assembler, encoder, frame, [lane]);
      if (assembler.progress().complete) break;
    }
    const done = assembler.takeCompleted();
    assert.ok(done !== null, `${lane} alone`);
    assert.deepEqual(done.payload, data);
  }
});

test("atoms seen before the first beacon are not lost", () => {
  const data = payload(300, 23);
  const encoder = new PetalStreamEncoder(data, 1);
  const assembler = new PetalStreamAssembler();
  for (let frame = 1; frame < 4; frame += 1) feedFrame(assembler, encoder, frame, ["P", "K", "D"]);
  assert.equal(assembler.progress().meta, null);
  for (let frame = 4; frame < encoder.systematicFrames() + 4; frame += 1) feedFrame(assembler, encoder, frame, ["P", "K", "D"]);
  assert.deepEqual(assembler.takeCompleted().payload, data);
});

test("joining mid-stream and losing frames still completes", () => {
  const data = payload(2500, 24);
  const encoder = new PetalStreamEncoder(data, 3);
  const assembler = new PetalStreamAssembler();
  const rng = new PetalXorshift32(77);
  let frame = 15;
  let shown = 0;
  while (!assembler.progress().complete) {
    if (rng.nextU32() % 10 < 6) feedFrame(assembler, encoder, frame, ["D", "P", "K"]);
    frame = (frame + 1) & 0xffff;
    shown += 1;
    assert.ok(shown < 600, "stream failed to complete");
  }
  assert.deepEqual(assembler.takeCompleted().payload, data);
});

test("a different stream replaces the active one after two beacons", () => {
  const first = new PetalStreamEncoder(payload(100, 1), 1);
  const second = new PetalStreamEncoder(payload(100, 2), 1);
  const assembler = new PetalStreamAssembler();
  feedFrame(assembler, first, 0, ["D"]);
  assert.deepEqual(assembler.progress().meta, first.meta);
  feedFrame(assembler, second, 0, ["D"]);
  assert.deepEqual(assembler.progress().meta, first.meta);
  feedFrame(assembler, second, 4, ["D"]);
  assert.deepEqual(assembler.progress().meta, second.meta);
});

test("a zero pending limit buffers nothing", () => {
  const data = payload(300, 41);
  const encoder = new PetalStreamEncoder(data, 1);
  const assembler = new PetalStreamAssembler({ maxPendingAtoms: 0 });
  // atoms before any beacon are dropped, not buffered
  for (let frame = 1; frame < 4; frame += 1) feedFrame(assembler, encoder, frame, ["P", "K", "D"]);
  feedFrame(assembler, encoder, 4, ["D"]);
  assert.equal(assembler.progress().rank, 0);
  assert.equal(assembler.progress().atomsReceived, 0);
});

test("oversized beacons are ignored", () => {
  const encoder = new PetalStreamEncoder(payload(4000, 5), 1);
  const assembler = new PetalStreamAssembler({ maxPayloadLen: 1000 });
  feedFrame(assembler, encoder, 0, ["D"]);
  assert.equal(assembler.progress().meta, null);
});

test("the pending-atom buffer is bounded and keeps the newest atoms", () => {
  const data = payload(600, 31);
  const encoder = new PetalStreamEncoder(data, 1);
  const assembler = new PetalStreamAssembler({ maxPendingAtoms: 3 });
  // frames 1..3 carry 21 atoms before any beacon; only the last three survive
  for (let frame = 1; frame < 4; frame += 1) feedFrame(assembler, encoder, frame, ["P", "K", "D"]);
  feedFrame(assembler, encoder, 0, ["D"]);
  assert.equal(assembler.progress().atomsReceived, 3);
  assert.equal(assembler.progress().rank, 3);
  // atoms of another stream (different tag) are buffered but never applied
  const other = new PetalStreamEncoder(payload(600, 32), 1);
  assert.notEqual(other.meta.tag, encoder.meta.tag);
  const fresh = new PetalStreamAssembler({ maxPendingAtoms: 128 });
  feedFrame(fresh, other, 1, ["P", "K"]);
  feedFrame(fresh, encoder, 0, ["D"]);
  assert.equal(fresh.progress().atomsReceived, 0);
});

test("corrupt atoms are caught by the payload CRC", () => {
  const data = payload(200, 25);
  const encoder = new PetalStreamEncoder(data, 1);
  const assembler = new PetalStreamAssembler();
  feedFrame(assembler, encoder, 0, ["D", "K"]);
  // atom 0 arrives with a valid header but a wrong body
  assembler.pushAtoms({ header: { tag: encoder.meta.tag, frame: 0 }, firstId: 0, atoms: [new Uint8Array(16).fill(0xee)] });
  for (let frame = 1; frame < encoder.systematicFrames(); frame += 1) feedFrame(assembler, encoder, frame, ["P", "K", "D"]);
  assert.equal(assembler.takeCompleted(), null);
  assert.equal(assembler.progress().integrityFailures, 1);
  // clean repair frames after the reset recover the payload
  for (let frame = 100; frame < 600; frame += 1) {
    feedFrame(assembler, encoder, frame, ["P", "K", "D"]);
    if (assembler.progress().complete) break;
  }
  assert.deepEqual(assembler.takeCompleted().payload, data);
});

test("the encoder rejects empty and oversized payloads", () => {
  assertPetalError(() => new PetalStreamEncoder(new Uint8Array(), 0), "empty_payload");
  assertPetalError(() => new PetalStreamEncoder(new Uint8Array(PETAL_STREAM.maxPayloadLen + 1), 0), "payload_too_large");
  assert.throws(() => new PetalStreamEncoder(new Uint8Array(4), 256), TypeError);
});

test("the beacon roundtrips through lane D", () => {
  const encoder = new PetalStreamEncoder(payload(77, 9), 3);
  const { d } = encoder.laneData(512);
  const lane = parseDLane(d);
  assert.equal(lane.type, "beacon");
  assert.deepEqual(lane.beacon.meta, encoder.meta);
  assert.equal(lane.beacon.header.frame, 512);
  assert.equal(parseDLane(d.slice(0, 11)), null);
  const bad = d.slice();
  bad[3] = 0x20;
  assert.equal(parseDLane(bad), null);
  assert.equal(parseDLane(encoder.laneData(513).d).type, "atoms");
  assert.equal(parseAtomLane("D", encoder.laneData(513).d), null);
});

test("random streams always complete and never deliver wrong data", () => {
  const rng = new PetalXorshift32(0xc0ffee11);
  for (let trial = 0; trial < 400; trial += 1) {
    // sizes cover K = 1, a few atoms, and a few hundred atoms
    const len =
      trial % 8 === 0 ? 1 + (rng.nextU32() % 16) : trial % 8 === 1 ? 17 + (rng.nextU32() % 100) : 1 + (rng.nextU32() % 3000);
    const data = Uint8Array.from({ length: len }, () => rng.nextByte());
    const kind = rng.nextByte();
    const encoder = new PetalStreamEncoder(data, kind);
    const lossPercent = rng.nextU32() % 70;
    const choice = rng.nextU32() % 5;
    const lanes = choice === 0 ? ["P"] : choice === 1 ? ["D", "P"] : choice === 2 ? ["K", "D"] : ["P", "K", "D"];
    const assembler = new PetalStreamAssembler();
    let frame = rng.nextU32() & 0xffff;
    let shown = 0;
    while (!assembler.progress().complete) {
      if (rng.nextU32() % 100 >= lossPercent) {
        // only lane D carries the beacon, so offer it on beacon frames
        const readable = isBeaconFrame(frame) && !lanes.includes("D") ? [...lanes, "D"] : lanes;
        feedFrame(assembler, encoder, frame, readable);
      }
      frame = (frame + 1) & 0xffff;
      shown += 1;
      const budget = 40 + Math.floor((8 * (Math.floor(len / 13) + 2) * 100) / (100 - lossPercent));
      assert.ok(shown < budget, `trial ${trial}: ${len} bytes, loss ${lossPercent} %, lanes ${lanes} exceeded ${budget} frames`);
    }
    const done = assembler.takeCompleted();
    assert.deepEqual(done.payload, data, `trial ${trial}`);
    assert.equal(done.meta.kind, kind);
  }
});

test("counter wraparound keeps atom ids consistent", () => {
  const data = Uint8Array.from({ length: 2000 }, (_, i) => (i * 7 + 3) & 0xff);
  const encoder = new PetalStreamEncoder(data, 1);
  const assembler = new PetalStreamAssembler();
  // start a few frames before the 16-bit counter wraps and run across it
  let frame = 65530;
  for (let step = 0; step < 200; step += 1) {
    feedFrame(assembler, encoder, frame, ["P", "K", "D"]);
    frame = (frame + 1) & 0xffff;
    if (assembler.progress().complete) break;
  }
  assert.deepEqual(assembler.takeCompleted().payload, data);
});

// ---------------------------------------------------------------- image and geometry

test("bilinear sampling interpolates between pixel centres", () => {
  const image = new PetalLuma(2, 1, Uint8Array.of(0, 100));
  assert.ok(Math.abs(image.sample(0.5, 0.5)) < 1e-9);
  assert.ok(Math.abs(image.sample(1.5, 0.5) - 100) < 1e-9);
  assert.ok(Math.abs(image.sample(1.0, 0.5) - 50) < 1e-9);
  assert.ok(Math.abs(image.sample(-5, 9)) < 1e-9);
});

test("strided planes drop the padding", () => {
  const plane = Uint8Array.of(1, 2, 9, 9, 3, 4, 9, 9);
  assert.deepEqual(Array.from(PetalLuma.fromStrided(2, 2, 4, plane).data), [1, 2, 3, 4]);
  assert.equal(PetalLuma.fromStrided(2, 2, 1, plane), null);
  assert.equal(PetalLuma.fromStrided(2, 3, 4, plane), null);
  assert.throws(() => new PetalLuma(2, 2, new Uint8Array(3)), RangeError);
});

test("RGBA luma uses the Rec. 601 weights", () => {
  const rgba = Uint8Array.of(255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255);
  assert.deepEqual(Array.from(PetalLuma.fromRgba(3, 1, rgba).data), [76, 150, 29]);
  const imageData = { width: 1, height: 2, data: new Uint8ClampedArray([10, 20, 30, 255, 200, 100, 50, 255]) };
  assert.deepEqual(Array.from(PetalLuma.fromImageData(imageData).data), [18, 124]);
  assert.equal(PetalLuma.fromRgba(2, 2, new Uint8Array(15)), null);
  // padded rows
  const padded = Uint8Array.of(255, 255, 255, 255, 7, 7, 0, 0, 0, 255, 7, 7);
  assert.deepEqual(Array.from(PetalLuma.fromRgba(1, 2, padded, 6).data), [255, 0]);
});

test("four points are mapped exactly by the homography fit", () => {
  const src = [
    [0, 0],
    [1024, 0],
    [1024, 1024],
    [0, 1024],
  ];
  const dst = [
    [103.5, 40.25],
    [590, 70],
    [560, 420],
    [80, 380],
  ];
  const h = PetalHomography.fromPoints(src, dst);
  src.forEach(([x, y], index) => {
    const [u, v] = h.apply(x, y);
    assert.ok(Math.abs(u - dst[index][0]) < 1e-7 && Math.abs(v - dst[index][1]) < 1e-7);
  });
});

test("the inverse roundtrips and least squares averages noise", () => {
  const truth = new PetalHomography([0.4, -0.1, 130, 0.12, 0.38, 60, 1e-4, -2e-5, 1]);
  const src = Array.from({ length: 30 }, (_, i) => [50 + 31 * (i % 6), 90 + 47 * Math.floor(i / 6)]);
  const dst = src.map(([x, y], i) => {
    const [u, v] = truth.apply(x, y);
    const jitter = i % 2 === 0 ? 0.05 : -0.05;
    return [u + jitter, v - jitter];
  });
  const fit = PetalHomography.fromPoints(src, dst);
  for (const [x, y] of src) {
    const [x0, y0] = truth.apply(x, y);
    const [x1, y1] = fit.apply(x, y);
    assert.ok(Math.abs(x0 - x1) < 0.1 && Math.abs(y0 - y1) < 0.1);
  }
  const [x, y] = truth.apply(300, 200);
  const [bx, by] = truth.inverse().apply(x, y);
  assert.ok(Math.abs(bx - 300) < 1e-6 && Math.abs(by - 200) < 1e-6);
  const composed = truth.compose(truth.inverse());
  const [cx, cy] = composed.apply(17, 23);
  assert.ok(Math.abs(cx - 17) < 1e-9 && Math.abs(cy - 23) < 1e-9);
});

test("degenerate homography inputs are rejected", () => {
  const p = [
    [1, 1],
    [1, 1],
    [1, 1],
    [1, 1],
  ];
  assert.equal(PetalHomography.fromPoints(p, p), null);
  assert.equal(PetalHomography.fromPoints(p.slice(0, 3), p.slice(0, 3)), null);
  assert.equal(new PetalHomography([0, 0, 0, 0, 0, 0, 0, 0, 0]).inverse(), null);
});

// ---------------------------------------------------------------- renderer

function renderCells(seed) {
  const p = Uint8Array.from({ length: 19 }, (_, b) => ((b * 31) & 0xff) + seed);
  const k = Uint8Array.from({ length: 83 }, (_, b) => ((b * 17) & 0xff) ^ seed);
  const d = Uint8Array.from({ length: 19 }, (_, b) => ((b * 13) & 0xff) ^ seed);
  return PetalFrameCells.fromWords(encodeLane("P", p), encodeLane("K", k), encodeLane("D", d));
}

test("finders are solid blossoms and corners are otherwise black", () => {
  const image = renderPetalFrame(renderCells(1), { size: 256, supersample: 2 });
  const at = (x, y) => image.data[(y * 256 + x) * 4];
  assert.ok(at(18, 18) > 200, "core must be lit");
  assert.ok(at(18, 9) > 200, "upper petal must be lit");
  assert.ok(at(23, 23) > 200, "blossom body must be lit");
  assert.equal(at(2, 255), 0);
  assert.equal(image.data[3], 255);
});

test("light tiles are bright and dark tiles are mostly black", () => {
  const frame = renderCells(2);
  const image = renderPetalFrame(frame, { size: 512, supersample: 2 });
  const light = [];
  const dark = [];
  for (let tile = 0; tile < 256; tile += 1) {
    const [cx, cy] = tileCenter(tile);
    const x0 = Math.floor((cx - 10) * 0.5);
    const y0 = Math.floor((cy - 10) * 0.5);
    let sum = 0;
    for (let j = 0; j < 10; j += 1) for (let i = 0; i < 10; i += 1) sum += image.data[((y0 + j) * 512 + x0 + i) * 4];
    (frame.light[tile] ? light : dark).push(sum / 100);
  }
  const mean = (values) => values.reduce((a, b) => a + b, 0) / values.length;
  assert.ok(mean(light) > mean(dark) + 25, `light ${mean(light)}, dark ${mean(dark)}`);
});

test("lit dots are drawn and unlit slots are black", () => {
  const frame = renderCells(3);
  const image = renderPetalFrame(frame, { size: 1024, supersample: 1 });
  let checked = 0;
  for (let ring = 0; ring < 3; ring += 1) {
    for (let slot = 0; slot < PETAL_LAYOUT.ringSlots[ring]; slot += 1) {
      const [x, y] = slotCenter(ring, slot);
      const value = image.data[(Math.floor(y) * 1024 + Math.floor(x)) * 4];
      const flat = [0, 80, 172][ring] + slot;
      if (frame.dots[flat]) assert.ok(value > 150, `ring ${ring} slot ${slot} should be lit`);
      else assert.equal(value, 0, `ring ${ring} slot ${slot} should be dark`);
      checked += 1;
    }
  }
  assert.equal(checked, 276);
});

test("the software renderer matches the reference renderer bit for bit", () => {
  // FNV-1a 64 of the RGB bytes produced by `iroha_petal::render::render` for
  // the decoder-test stream (300 bytes, kind 2).
  const fnv = (bytes) => {
    // 64-bit FNV-1a on two 32-bit halves; the prime is 2^40 + 0x1b3.
    let hi = 0xcbf29ce4;
    let lo = 0x84222325;
    for (const byte of bytes) {
      lo = (lo ^ byte) >>> 0;
      const product = lo * 0x1b3;
      hi = (Math.imul(hi, 0x1b3) + Math.floor(product / 2 ** 32) + ((lo << 8) >>> 0)) >>> 0;
      lo = product >>> 0;
    }
    return hi.toString(16).padStart(8, "0") + lo.toString(16).padStart(8, "0");
  };
  const data = Uint8Array.from({ length: 300 }, (_, i) => (Math.imul(i, 2654435761) >>> 11) & 0xff);
  const encoder = new PetalStreamEncoder(data, 2);
  for (const [frame, size, supersample, expected] of [
    [0, 256, 2, "08ef9eae80716bd1"],
    [3, 1024, 1, "2fa730d9d1f440d1"],
  ]) {
    const image = renderPetalFrame(encoder.cells(frame), { size, supersample });
    const rgb = new Uint8Array(size * size * 3);
    for (let pixel = 0; pixel < size * size; pixel += 1) rgb.set(image.data.subarray(pixel * 4, pixel * 4 + 3), pixel * 3);
    assert.equal(fnv(rgb), expected, `frame ${frame} at ${size}px`);
  }
});

test("the draw list reproduces the software renderer's geometry", () => {
  const cells = renderCells(4);
  const list = petalDrawList(cells);
  assert.equal(list.canvas, 1024);
  assert.equal(list.finders.length, 4);
  assert.equal(list.tiles.length, 256);
  assert.equal(list.dots.length, cells.dots.reduce((a, b) => a + b, 0));
  assert.equal(list.strokeWidth, (6.5 * 23) / 32);
  for (const finder of list.finders) {
    assert.equal(finder.core.r, 12);
    assert.equal(finder.petals.length, 5);
    assert.equal(finder.notches.length, 5);
  }
  const image = renderPetalFrame(cells, { size: 1024, supersample: 1 });
  const palette = list.palette;
  const inCircle = (c, x, y) => Math.hypot(x - c.x, y - c.y) <= c.r;
  const segment = (px, py, ax, ay, bx, by) => {
    const dx = bx - ax;
    const dy = by - ay;
    const t = Math.min(1, Math.max(0, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)));
    return Math.hypot(px - (ax + t * dx), py - (ay + t * dy));
  };
  const inRounded = (tile, x, y) => {
    const ax = Math.abs(x - (tile.x + tile.size / 2));
    const ay = Math.abs(y - (tile.y + tile.size / 2));
    const half = tile.size / 2;
    if (ax > half || ay > half) return false;
    const cx = ax - (half - tile.radius);
    const cy = ay - (half - tile.radius);
    return cx <= 0 || cy <= 0 || cx * cx + cy * cy <= tile.radius * tile.radius;
  };
  const byCell = new Map(list.tiles.map((tile) => [PETAL_LAYOUT.tiles[tile.index].join(","), tile]));
  const tileAt = (x, y) => byCell.get(`${Math.floor((x - 222) / 29)},${Math.floor((y - 222) / 29)}`);
  const colorAt = (x, y) => {
    let color = palette.background;
    for (const finder of list.finders) {
      if (inCircle(finder.core, x, y) || finder.petals.some((c) => inCircle(c, x, y))) color = palette.light;
      if (finder.notches.some((c) => inCircle(c, x, y))) color = palette.background;
    }
    const tile = tileAt(x, y);
    if (tile !== undefined) {
      if (tile.light && inRounded(tile, x, y)) color = tile.fill;
      const box = tile.glyphBox;
      if (x > box.x && x < box.x + box.size && y > box.y && y < box.y + box.size) {
        for (const stroke of tile.strokes) {
          for (let i = 0; i + 3 < stroke.length; i += 2) {
            if (segment(x, y, stroke[i], stroke[i + 1], stroke[i + 2], stroke[i + 3]) <= list.strokeWidth / 2) color = tile.ink;
          }
        }
      }
    }
    const radius = Math.hypot(x - 512, y - 512);
    if (radius >= 360 - 12 && radius <= 460 + 12) {
      for (const dot of list.dots) if (inCircle(dot, x, y)) color = palette.pink;
    }
    return color;
  };
  const inGlyphBox = (x, y) => {
    const tile = tileAt(x, y);
    if (tile === undefined) return false;
    const box = tile.glyphBox;
    return x > box.x && x < box.x + box.size && y > box.y && y < box.y + box.size;
  };
  let glyphSamples = 0;
  let glyphMismatches = 0;
  let otherMismatches = 0;
  for (let py = 0; py < 1024; py += 3) {
    for (let px = 0; px < 1024; px += 3) {
      const expected = colorAt(px + 0.5, py + 0.5);
      const at = (py * 1024 + px) * 4;
      const differs = image.data[at] !== expected[0] || image.data[at + 1] !== expected[1] || image.data[at + 2] !== expected[2];
      if (inGlyphBox(px + 0.5, py + 0.5)) {
        glyphSamples += 1;
        if (differs) glyphMismatches += 1;
      } else if (differs) {
        otherMismatches += 1;
      }
    }
  }
  // Finders, tiles and dots match exactly. Glyph outlines may differ by a
  // hair: the renderer quantises glyph ink to a 128-cell bitmap per glyph
  // box, the draw list keeps the exact strokes.
  assert.equal(otherMismatches, 0);
  assert.ok(glyphMismatches / glyphSamples < 0.02, `${glyphMismatches} of ${glyphSamples} glyph samples differ`);
  assert.deepEqual(list.palette, PETAL_PALETTE);
});

// ---------------------------------------------------------------- locate

test("the four corner blossoms are found in a clean render", () => {
  const encoder = new PetalStreamEncoder(new Uint8Array(200).fill(9), 1);
  const quad = locate(renderLuma(encoder.cells(1), 512, 2));
  const expected = [
    [36, 36],
    [476, 36],
    [476, 476],
    [36, 476],
  ];
  quad.forEach((finder, index) => {
    assert.ok(Math.abs(finder.x - expected[index][0]) < 1.5 && Math.abs(finder.y - expected[index][1]) < 1.5, JSON.stringify(finder));
    assert.ok(Math.abs(finder.size - 60) < 6, `size ${finder.size}`);
  });
});

test("components are labelled with the correct geometry", () => {
  const mask = new Uint8Array(64);
  for (let y = 1; y < 4; y += 1) for (let x = 2; x < 6; x += 1) mask[y * 8 + x] = 1;
  mask[6 * 8 + 6] = 1;
  const components = labelComponents(mask, 8, 8);
  assert.equal(components.length, 2);
  const big = components.find((c) => c.area === 12);
  assert.deepEqual([big.minX, big.maxX, big.minY, big.maxY], [2, 5, 1, 3]);
  assert.ok(Math.abs(big.sumX / big.area - 4) < 1e-9 && Math.abs(big.sumY / big.area - 2.5) < 1e-9);
  assert.deepEqual(blossoms(components), []);
});

test("finder quads are ordered clockwise from the top left", () => {
  const f = (x, y) => ({ x, y, size: 10 });
  const quad = selectQuad([f(90, 90), f(10, 12), f(88, 8), f(12, 92)]);
  assert.deepEqual([quad[0].x, quad[0].y], [10, 12]);
  assert.deepEqual([quad[1].x, quad[1].y], [88, 8]);
  assert.deepEqual([quad[2].x, quad[2].y], [90, 90]);
  assert.equal(selectQuad([f(1, 1), f(2, 2), f(3, 3)]), null);
});

test("the largest candidates win when clutter precedes them", () => {
  const blob = (x, y, size) => ({ x, y, size });
  // twelve small decoys discovered before the four real finders
  const real = [blob(100, 100, 60), blob(700, 110, 62), blob(690, 520, 58), blob(95, 510, 61)];
  const small = Array.from({ length: 12 }, (_, i) => blob(10 + 7 * i, 5, 18));
  const quad = selectQuad([...small, ...real]);
  assert.deepEqual(quad.map((finder) => Math.trunc(finder.x)).sort((a, b) => a - b), [95, 100, 690, 700]);
  // decoys large enough to pass the size-class filter: only ranking the
  // candidates largest first keeps the real finders among the ten combined
  const large = Array.from({ length: 12 }, (_, i) => blob(10 + 7 * i, 5, 40));
  assert.deepEqual(selectQuad([...large, ...real]), real);
});

test("a blank image has no finders", () => {
  assert.equal(locate(new PetalLuma(200, 200)), null);
});

// ---------------------------------------------------------------- frame decoder

function decodeSetup(frame) {
  const data = Uint8Array.from({ length: 300 }, (_, i) => (Math.imul(i, 2654435761) >>> 11) & 0xff);
  const encoder = new PetalStreamEncoder(data, 2);
  return { encoder, luma: renderLuma(encoder.cells(frame), 768, 2) };
}

function transformLuma(luma, map) {
  const n = luma.width;
  const out = new Uint8Array(n * n);
  for (let y = 0; y < n; y += 1) {
    for (let x = 0; x < n; x += 1) {
      const [sx, sy] = map(x, y, n);
      out[y * n + x] = luma.data[sy * n + sx];
    }
  }
  return new PetalLuma(n, n, out);
}

test("a clean render decodes every lane", () => {
  const { encoder, luma } = decodeSetup(5);
  const decoded = decodePetalFrame(luma);
  const { p, k, d } = encoder.laneData(5);
  assert.deepEqual(decoded.p.data, p);
  assert.deepEqual(decoded.k.data, k);
  assert.deepEqual(decoded.d.data, d);
  assert.deepEqual([decoded.rotation, decoded.mirrored, decoded.lanesOk()], [0, false, 3]);
  assert.equal(decoded.beacon(), null);
  assert.equal(decoded.atomPackets().length, 3);
  assert.ok(tileMatchError(luma, decoded) < 0.5);
  assert.ok(observedCells(luma, decoded).equals(encoder.cells(5)));
  const at = decodePetalFrameAt(luma, decoded.homography);
  assert.deepEqual(at.p.data, p);
});

test("rotated and mirrored renders decode with the right orientation", () => {
  const { encoder, luma } = decodeSetup(7);
  const { p, d } = encoder.laneData(7);
  const cases = [
    ["rot90", (x, y, n) => [y, n - 1 - x], false],
    ["rot180", (x, y, n) => [n - 1 - x, n - 1 - y], false],
    ["rot270", (x, y, n) => [n - 1 - y, x], false],
    ["mirror", (x, y, n) => [n - 1 - x, y], true],
  ];
  const rotations = [];
  for (const [name, map, mirrored] of cases) {
    const decoded = decodePetalFrame(transformLuma(luma, map));
    assert.equal(decoded.mirrored, mirrored, name);
    assert.deepEqual(decoded.d.data, d, `${name} lane D`);
    assert.deepEqual(decoded.p.data, p, `${name} lane P`);
    rotations.push(decoded.rotation);
  }
  // orientations reported by the reference decoder for these images (a
  // mirrored hypothesis walks the finders counter-clockwise, so a plain
  // mirror is reported as rotation 1)
  assert.deepEqual(rotations, [1, 2, 3, 1]);
});

test("corrected counts rewritten bytes, not just erasures", () => {
  const data = Uint8Array.from({ length: 19 }, (_, b) => b);
  const word = encodeLane("P", data);
  for (const position of [2, 11, 30]) word[position] ^= 0x5a;
  const result = decodeWithErasures("P", word, new Float64Array(word.length).fill(1));
  assert.deepEqual(result.data, data);
  assert.equal(result.erasures, 0);
  assert.equal(result.corrected, 3);
  // with the damaged bytes flagged as least confident, they become erasures
  const flagged = new Float64Array(word.length).fill(1);
  for (const position of [2, 11, 30]) flagged[position] = 0;
  const again = decodeWithErasures("P", word, flagged);
  assert.deepEqual(again.data, data);
  assert.ok(again.corrected >= 3);
  // 8 bad bytes exceed the 6-error capacity of 13 parity bytes: the schedule
  // (0, 1, 3, ... erasures) first succeeds with 3 erasures and 5 found errors,
  // as the reference's counted decoder reports
  const heavy = encodeLane("P", data);
  const bad = [1, 4, 9, 12, 17, 22, 26, 29];
  for (const position of bad) heavy[position] ^= 0xa7;
  const confidence = new Float64Array(heavy.length).fill(1);
  for (const position of bad) confidence[position] = 0;
  const erased = decodeWithErasures("P", heavy, confidence);
  assert.deepEqual(erased.data, data);
  assert.equal(erased.erasures, 3);
  assert.equal(erased.corrected, 8);
});

test("random words are almost never accepted", () => {
  // Reed-Solomon with erasures can accept a word that is not a transmission. Lane D has only 11
  // parity bytes, so its schedule stops at five erasures; at seven it let through about one random
  // word in 250 (150 of these 40 000). The counts are exact so that every SDK port, fed the same
  // xorshift32 words and byte-valued confidences (many ties, so the ranking must be stable),
  // reproduces the decoder bit for bit.
  const rng = new PetalXorshift32(0x5eed);
  const trials = 40000;
  for (const [lane, expected] of [
    ["D", 3],
    ["P", 0],
  ]) {
    const length = PETAL_LANES[lane].wordLen;
    let accepted = 0;
    for (let trial = 0; trial < trials; trial += 1) {
      const word = new Uint8Array(length);
      for (let index = 0; index < length; index += 1) word[index] = rng.nextByte();
      const confidence = new Float64Array(length);
      for (let index = 0; index < length; index += 1) confidence[index] = rng.nextByte();
      if (decodeWithErasures(lane, word, confidence) !== null) accepted += 1;
    }
    assert.equal(accepted, expected, `lane ${lane} of ${trials} random words`);
  }
});

test("only lane K uses two thirds of its parity as erasures", () => {
  // Damaged bytes: `flagged` of them marked least confident, two more hidden. With the extra
  // erasure step of the old schedule the decoder would repair them (2 * 2 + flagged parity
  // bytes); the capped schedule must refuse instead of risking a wrong codeword.
  for (const [lane, flagged] of [
    ["D", 7],
    ["P", 8],
  ]) {
    const { dataLen, parityLen, wordLen } = PETAL_LANES[lane];
    const data = Uint8Array.from({ length: dataLen }, (_, index) => index);
    const word = encodeLane(lane, data);
    const damaged = word.slice();
    const confidence = new Float64Array(wordLen).fill(1);
    for (let position = 0; position < flagged; position += 1) {
      damaged[position] ^= 0xa5;
      confidence[position] = 0;
    }
    damaged[20] ^= 0x3c;
    damaged[21] ^= 0x3c;
    assert.equal(decodeWithErasures(lane, damaged, confidence), null, `lane ${lane}`);
    // half the parity flagged plus one hidden error stays comfortably repairable
    const half = Math.floor(parityLen / 2);
    const repairable = word.slice();
    const halfFlagged = new Float64Array(wordLen).fill(1);
    for (let position = 0; position < half; position += 1) {
      repairable[position] ^= 0xa5;
      halfFlagged[position] = 0;
    }
    repairable[20] ^= 0x3c;
    const result = decodeWithErasures(lane, repairable, halfFlagged);
    assert.deepEqual(result.data, data, `lane ${lane}`);
    assert.ok(result.erasures <= half, `lane ${lane}`);
  }
  // lane K keeps the two-thirds step: 30 flagged bytes plus 7 hidden errors need it
  // (2 * 7 + 30 = 44 of 45 parity bytes)
  const data = Uint8Array.from({ length: PETAL_LANES.K.dataLen }, (_, index) => index);
  const damaged = encodeLane("K", data);
  const confidence = new Float64Array(damaged.length).fill(1);
  for (let position = 0; position < 30; position += 1) {
    damaged[position] ^= 0xa5;
    confidence[position] = 0;
  }
  for (let position = 60; position < 67; position += 1) damaged[position] ^= 0x3c;
  const result = decodeWithErasures("K", damaged, confidence);
  assert.deepEqual([result.data, result.erasures], [data, 30]);
});

test("the erasure schedule has the reference's steps for every lane", () => {
  // `flagged` damaged bytes are ranked least confident, `hidden` ones look fine; a lane decodes at
  // the first step whose erasures plus twice the remaining errors fit the parity. The cases below
  // fail at every earlier step, so the step that succeeds is the one that is pinned.
  const cases = [
    // lane, flagged, hidden, erasures used (null: no step of the schedule can repair it)
    ["D", 5, 3, 5], // steps 0, 1, 2, 3, 5: the half step is the last one
    ["D", 7, 2, null], // a seventh erasure would repair it, the capped schedule must not
    ["P", 7, 2, 6], // steps 0, 1, 3, 4, 6: the half step is the last one for P too
    ["P", 8, 2, null], // an eighth erasure would repair it
    ["K", 30, 7, 30], // steps 0, 5, 11, 15, 22, 30: only K has the two-thirds step
    ["K", 31, 7, null], // nothing beyond the two-thirds step
  ];
  for (const [lane, flagged, hidden, erasures] of cases) {
    const { dataLen, wordLen } = PETAL_LANES[lane];
    const data = Uint8Array.from({ length: dataLen }, (_, index) => (index * 7 + 3) & 0xff);
    const damaged = encodeLane(lane, data);
    const confidence = new Float64Array(wordLen).fill(1);
    for (let position = 0; position < flagged; position += 1) {
      damaged[position] ^= 0xa5;
      confidence[position] = 0;
    }
    for (let position = wordLen - hidden; position < wordLen; position += 1) damaged[position] ^= 0x3c;
    const result = decodeWithErasures(lane, damaged, confidence);
    const label = `lane ${lane}, ${flagged} flagged and ${hidden} hidden`;
    if (erasures === null) {
      assert.equal(result, null, label);
    } else {
      assert.deepEqual(result.data, data, label);
      assert.equal(result.erasures, erasures, label);
      assert.equal(result.corrected, flagged + hidden, label);
    }
  }
});

test("equal confidences are erased in position order, so the ranking is stable", () => {
  // The random-word counts above do not depend on the order of tied confidences (the same counts
  // come out with the ties reversed), but real reads tie a lot: every tile the normalised read
  // erases has confidence exactly 0. With all confidences equal the lowest positions are erased
  // first, so damage at the lowest positions is repaired as soon as the schedule reaches enough
  // erasures; a ranking that put the ties the other way round would erase good bytes instead.
  for (const [lane, damagedPositions, mask, erasures] of [
    // The reference's case: three damaged bytes at the front plus four hidden ones fit lane D only
    // if exactly the first three positions are erased (3 erasures + 4 errors = all 11 parity bytes);
    // a step that erased the last positions instead would see seven errors.
    ["D", [0, 1, 2, 20, 21, 22, 23], 0x5a, 3],
    ["P", [0, 1, 2, 3, 4, 5, 6, 7], 0xa5, 3], // 13 parity bytes: 8 errors fail, 3 erasures + 5 errors fit
    ["D", [0, 1, 2, 3, 4, 5], 0xa5, 1], // 11 parity bytes: 6 errors fail, 1 erasure + 5 errors fit
  ]) {
    const { dataLen, wordLen } = PETAL_LANES[lane];
    const data = Uint8Array.from({ length: dataLen }, (_, index) => index);
    const word = encodeLane(lane, data);
    for (const position of damagedPositions) word[position] ^= mask;
    const result = decodeWithErasures(lane, word, new Float64Array(wordLen).fill(1));
    const label = `lane ${lane}, damaged ${damagedPositions.join(",")}`;
    assert.ok(result !== null, label);
    assert.deepEqual([result.data, result.erasures, result.corrected], [data, erasures, damagedPositions.length], label);
  }
});

test("blank frames report no finders", () => {
  assertPetalError(() => decodePetalFrame(new PetalLuma(320, 240)), "no_finders");
});

test("unusable sizes are rejected without work", () => {
  assertPetalError(() => decodePetalFrame(new PetalLuma(1, 1)), "unsupported_image");
  assertPetalError(() => decodePetalFrame(new PetalLuma(47, 400)), "unsupported_image");
  assertPetalError(() => decodePetalFrame(new PetalLuma(0, 0)), "unsupported_image");
  assertPetalError(() => decodePetalFrame(new PetalLuma(100, 100), { maxPixels: 1000 }), "unsupported_image");
  assertPetalError(() => decodePetalFrame({ width: 100, height: 100, data: new Uint8Array(5) }), "unsupported_image");
  // huge frames are refused before any pixel is touched
  assertPetalError(() => decodePetalFrame({ width: 100000, height: 100000, data: new Uint8Array(0) }), "unsupported_image");
  assertPetalError(() => decodePetalFrame({ width: 4001, height: 3000, data: new Uint8Array(0) }), "unsupported_image");
  assert.equal(decodePetalFrameAt(new PetalLuma(8, 8), PetalHomography.identity()), null);
  // a buffer that does not match the stated size must not reach the sampler, in any entry point
  const broken = { width: 100, height: 100, data: new Uint8Array(5) };
  assert.equal(decodePetalFrameAt(broken, PetalHomography.identity()), null);
  const { luma } = decodeSetup(5);
  const decoded = decodePetalFrame(luma);
  assert.equal(observedCells(broken, decoded), null);
  assert.equal(tileMatchError(broken, decoded), null);
  // ... and so must a frame that is too small or over the pixel budget
  for (const [image, options] of [
    [new PetalLuma(47, 400), undefined],
    [luma, { maxPixels: 1000 }],
  ]) {
    assert.equal(observedCells(image, decoded, options), null);
    assert.equal(tileMatchError(image, decoded, options), null);
    assert.equal(decodePetalFrameAt(image, decoded.homography, options), null);
  }
  assert.throws(() => decodePetalFrame({ width: 64, height: 64, data: [] }), TypeError);
  assert.throws(() => decodePetalFrame(new PetalLuma(64, 64), { templateSigmas: [] }), TypeError);
});

test("garbage images never crash or decode", () => {
  const rng = new PetalXorshift32(99);
  for (const [w, h] of [
    [64, 48],
    [257, 129],
    [320, 240],
    [480, 480],
  ]) {
    for (let style = 0; style < 4; style += 1) {
      const data = new Uint8Array(w * h);
      for (let i = 0; i < w * h; i += 1) {
        if (style === 0) data[i] = rng.nextByte(); // white noise
        else if (style === 1) data[i] = Math.floor(((i % w) * 255) / w); // gradient
        else if (style === 2) data[i] = (Math.floor(Math.floor(i / w) / 8) + Math.floor((i % w) / 8)) % 2 === 0 ? 230 : 20; // checkerboard
        else data[i] = rng.nextU32() % 50 === 0 ? 255 : 0; // sparse specks
      }
      assert.throws(() => decodePetalFrame(new PetalLuma(w, h, data)), PetalError, `${w}x${h} style ${style}`);
    }
  }
  // tiny, extreme-aspect and vertical-gradient frames, plus degenerate poses
  for (const [w, h] of [
    [48, 48],
    [48, 2000],
    [2000, 48],
  ]) {
    const data = Uint8Array.from({ length: w * h }, (_, i) => Math.floor((Math.floor(i / w) * 255) / h));
    assert.throws(() => decodePetalFrame(new PetalLuma(w, h, data)), PetalError);
  }
  // NaN and infinite poses behave as in the reference: no crash, no lanes
  const noisy = new PetalLuma(320, 240, payload(320 * 240, 5));
  const lanes = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1e300, 0, 0, 0, 1e300, 0, 0, 0, 1e-300],
    [Number.NaN, 0, 0, 0, 1, 0, 0, 0, 1],
  ].map((m) => decodePetalFrameAt(noisy, m)?.lanesOk() ?? null);
  assert.deepEqual(lanes, [0, null, 0]);
});

test("a valid code with a missing finder is not misread", () => {
  const { luma } = decodeSetup(2);
  const n = luma.width;
  const data = luma.data.slice();
  for (let y = (n * 3) / 4; y < n; y += 1) for (let x = (n * 3) / 4; x < n; x += 1) data[y * n + x] = 0;
  assert.throws(() => decodePetalFrame(new PetalLuma(n, n, data)), PetalError);
});

// ---------------------------------------------------------------- tile reads

const SIGMAS = PETAL_DEFAULT_DECODE_OPTIONS.templateSigmas;
/** Cells in the patch of one tile. */
const PATCH_CELLS = PETAL_GLYPHS.templateN * PETAL_GLYPHS.templateN;

/** The exact canvas-to-pixel homography (nine coefficients) of the 768-pixel test renders. */
function renderHomography() {
  const canonical = PETAL_LAYOUT.finderCenters;
  const scale = 768 / PETAL_LAYOUT.canvas;
  const pixels = canonical.map(([x, y]) => [x * scale, y * scale]);
  return Float64Array.from(PetalHomography.fromPoints(canonical, pixels).values);
}

/** A clean 768-pixel render with the exact canvas-to-pixel homography and its raw patches. */
function cleanPatches(frame) {
  const { encoder, luma } = decodeSetup(frame);
  const m = renderHomography();
  return { encoder, luma, m, patches: samplePatches(luma, m) };
}

function decodedLane(lane, words) {
  const key = lane.toLowerCase();
  return decodeWithErasures(lane, words[key], words[`${key}Confidence`]);
}

/** Contrast (span between the robust darkest and brightest cell) of every tile's patch. */
function patchSpans(patches) {
  return Array.from({ length: PETAL_LAYOUT.tileCount }, (_, tile) => {
    const [low, high] = patchLevels(patches, tile * PATCH_CELLS);
    return high - low;
  });
}

/** Scales the contrast of one tile's patch about its own darkest level; the span scales by `gain`. */
function squeezeTile(patches, tile, gain) {
  const base = tile * PATCH_CELLS;
  const [low] = patchLevels(patches, base);
  for (let cell = 0; cell < PATCH_CELLS; cell += 1) {
    patches[base + cell] = low + gain * (patches[base + cell] - low);
  }
}

/** Remaps one tile's patch linearly so that its robust darkest level is 0 and its span exactly `span`. */
function setTileSpan(patches, tile, span) {
  const base = tile * PATCH_CELLS;
  const [low, high] = patchLevels(patches, base);
  for (let cell = 0; cell < PATCH_CELLS; cell += 1) {
    patches[base + cell] = ((patches[base + cell] - low) / (high - low)) * span;
  }
}

/**
 * Flattens fourteen tiles that sit in fourteen different bytes of lane P (and of lane K): more
 * damage than the 13 parity bytes of lane P repair, which lane K (45 parity bytes) shrugs off.
 * Returns the tile numbers.
 */
function flattenFourteenTiles(patches) {
  const tiles = Array.from({ length: 14 }, (_, j) => 8 * j);
  for (const tile of tiles) patches.fill(100, tile * PATCH_CELLS, (tile + 1) * PATCH_CELLS);
  return tiles;
}

const isErased = (reads, tile) => reads.polarityMargin[tile] === 0 && reads.glyphMargin[tile] === 0;

test("level and normalised reads agree on a clean render", () => {
  const { encoder, luma, m, patches } = cleanPatches(5);
  const { p: pData, k: kData } = encoder.laneData(5);
  const reference = referenceLevels(luma, m);
  assert.ok(reference !== null, "reference levels");
  for (const [name, reads] of [
    ["level", readTiles(patches, reference, SIGMAS)],
    ["normalised", readTilesNormalised(patches, SIGMAS)],
  ]) {
    const words = tileWords(reads);
    const p = decodedLane("P", words);
    const k = decodedLane("K", words);
    assert.deepEqual([p.data, p.corrected], [pData, 0], name);
    assert.deepEqual([k.data, k.corrected], [kData, 0], name);
  }
});

test("the normalised read cancels gain and offset per tile", () => {
  const { encoder, luma, m, patches } = cleanPatches(5);
  const { p: pData, k: kData } = encoder.laneData(5);
  // every tile gets its own gain and offset, as under glare, shadows and saturation
  const distorted = new Float64Array(patches.length);
  for (let tile = 0; tile < PETAL_LAYOUT.tileCount; tile += 1) {
    const gain = 0.35 + 0.65 * (((tile * 37) % 101) / 100);
    const offset = 5 + ((tile * 53) % 61);
    for (let cell = 0; cell < PATCH_CELLS; cell += 1) {
      distorted[tile * PATCH_CELLS + cell] = gain * patches[tile * PATCH_CELLS + cell] + offset;
    }
  }
  const reference = referenceLevels(luma, m);
  assert.equal(
    decodedLane("P", tileWords(readTiles(distorted, reference, SIGMAS))),
    null,
    "the level read must not survive this distortion, or the test proves nothing",
  );
  const words = tileWords(readTilesNormalised(distorted, SIGMAS));
  assert.deepEqual([decodedLane("P", words).data, decodedLane("K", words).data], [pData, kData]);
});

test("the normalised read erases tiles that lost their contrast", () => {
  const { patches } = cleanPatches(5);
  patches.fill(100, 5 * PATCH_CELLS, 6 * PATCH_CELLS);
  patches.fill(30, 9 * PATCH_CELLS, 10 * PATCH_CELLS);
  const reads = readTilesNormalised(patches, SIGMAS);
  for (const tile of [5, 9]) {
    assert.ok(Math.abs(reads.polarityMargin[tile]) < 1e-12, `tile ${tile}`);
    assert.ok(Math.abs(reads.glyphMargin[tile]) < 1e-12, `tile ${tile}`);
  }
  assert.ok(reads.polarityMargin[6] > 0 && reads.glyphMargin[6] > 0);
});

test("tiles under a quarter of the median contrast are erased, others are not", () => {
  const { patches } = cleanPatches(5);
  const spans = patchSpans(patches);
  const ascending = spans.map((span, tile) => [span, tile]).sort((a, b) => a[0] - b[0]);
  const median = ascending[PETAL_LAYOUT.tileCount / 2][0];
  // Two tiles below the median: squeezing them cannot move the median itself.
  const [weak, fine] = [ascending[10][1], ascending[20][1]];
  squeezeTile(patches, weak, (0.24 * median) / spans[weak]);
  squeezeTile(patches, fine, (0.26 * median) / spans[fine]);
  // Exactly a quarter of the median is still read (the comparison is strict); a hair under is not.
  const [exact, hairUnder] = [ascending[30][1], ascending[40][1]];
  setTileSpan(patches, exact, 0.25 * median);
  setTileSpan(patches, hairUnder, 0.25 * median * (1 - 2 ** -40));
  const reads = readTilesNormalised(patches, SIGMAS);
  assert.ok(isErased(reads, weak), "0.24 x median is erased");
  assert.ok(reads.polarityMargin[fine] > 0 && reads.glyphMargin[fine] > 0, "0.26 x median is read");
  assert.ok(reads.polarityMargin[exact] > 0 && reads.glyphMargin[exact] > 0, "exactly 0.25 x median is read");
  assert.ok(isErased(reads, hairUnder), "just under 0.25 x median is erased");
  const all = Array.from({ length: PETAL_LAYOUT.tileCount }, (_, tile) => tile);
  const erased = all.filter((tile) => isErased(reads, tile));
  assert.deepEqual(erased, [weak, hairUnder].sort((a, b) => a - b), "no other tile is erased");
});

test("the median contrast is the upper middle of the sorted tile spans", () => {
  // Half of the tiles keep a tenth of their contrast. Sorted, the upper middle span (element 128
  // of the ascending spans) belongs to a strong tile, so the squeezed half falls under a quarter
  // of it; the lower middle span would not erase them, and neither would the span of tile 128 when
  // that tile is among the squeezed ones (the spans are not in tile order).
  const half = PETAL_LAYOUT.tileCount / 2;
  for (const [name, squeezed] of [
    ["lower half", (tile) => tile < half],
    ["upper half", (tile) => tile >= half],
  ]) {
    const { patches } = cleanPatches(5);
    for (let tile = 0; tile < PETAL_LAYOUT.tileCount; tile += 1) {
      if (squeezed(tile)) squeezeTile(patches, tile, 0.1);
    }
    const reads = readTilesNormalised(patches, SIGMAS);
    for (let tile = 0; tile < PETAL_LAYOUT.tileCount; tile += 1) {
      assert.equal(isErased(reads, tile), squeezed(tile), `${name}: tile ${tile}`);
    }
  }
});

test("patch levels ignore the extreme cells", () => {
  const values = new Float64Array(PATCH_CELLS).fill(10);
  for (let i = 0; i < PATCH_CELLS / 2; i += 1) values[i] = 200 + (i % 3);
  values[0] = 255; // one hot cell
  values[PATCH_CELLS - 1] = 0; // one dead cell
  const [low, high] = patchLevels(values);
  assert.ok(Math.abs(low - 10) < 1e-12);
  assert.ok(high >= 200 && high <= 202);
  // the cut is exactly six cells in from either end: indices 6 and 57 of the 64 sorted cells
  const permuted = Float64Array.from({ length: PATCH_CELLS }, (_, cell) => (cell * 37) % PATCH_CELLS);
  assert.deepEqual(patchLevels(permuted), [6, 57]);
  const scaled = new Float64Array(PATCH_CELLS);
  assert.equal(rescale(values, 0, 1, scaled), high - low, "rescale reports the span it used");
  assert.ok(scaled.every((value) => value >= -0.25 && value <= 1.25));
  assert.equal(scaled[0], 1.25, "the hot cell is clamped");
  assert.ok(Math.abs(scaled[PATCH_CELLS - 1] - (0 - low) / (high - low)) < 1e-12, "the dead cell is not");
  // patches are addressed by offset and may be rescaled in place
  const stacked = new Float64Array(2 * PATCH_CELLS);
  stacked.set(values, PATCH_CELLS);
  assert.deepEqual(patchLevels(stacked, PATCH_CELLS), [low, high]);
  rescale(stacked, PATCH_CELLS, 1, stacked);
  assert.deepEqual(stacked.slice(PATCH_CELLS), scaled);
  assert.ok(stacked.subarray(0, PATCH_CELLS).every((value) => value === 0), "the neighbouring patch is untouched");
  // a cell far below the darkest level is clamped from below; the two levels map to 0 and 1
  const stepped = new Float64Array(PATCH_CELLS).fill(100);
  stepped.fill(110, 0, 20);
  stepped[PATCH_CELLS - 1] = 0;
  const steppedOut = new Float64Array(PATCH_CELLS);
  assert.equal(rescale(stepped, 0, 1, steppedOut), 10);
  assert.deepEqual([steppedOut[0], steppedOut[19], steppedOut[20], steppedOut[PATCH_CELLS - 1]], [1, 1, 0, -0.25]);
  // a flat patch stays flat instead of dividing by nothing
  const flatOut = new Float64Array(PATCH_CELLS);
  assert.equal(rescale(new Float64Array(PATCH_CELLS).fill(7), 0, 1, flatOut), 0);
  assert.ok(flatOut.every((value) => Math.abs(value) < 1e-12));
});

/**
 * The expected patch of every hypothesis `polarity * 16 + glyph` for the sharp template (sigma 0):
 * levels relative to the light fill, with the ink at 0.04 on a light tile and the pink glyph at
 * 0.83 on a dark tile (spec section 7, step 5).
 */
function templatePatches() {
  const hypotheses = [];
  for (let polarity = 0; polarity < 2; polarity += 1) {
    for (let glyph = 0; glyph < PETAL_GLYPHS.count; glyph += 1) {
      hypotheses.push(
        Float64Array.from(PETAL_GLYPHS.templates[glyph], (byte) => {
          const coverage = byte / 255;
          return polarity === 1 ? 1 - (1 - 0.04) * coverage : 0.83 * coverage;
        }),
      );
    }
  }
  return hypotheses;
}

test("both reads recognise every template at any gain and offset", () => {
  const hypotheses = templatePatches();
  // Level read: one gain and offset for the whole frame, which the finder levels describe exactly.
  const [gain, offset] = [200, 20];
  const levelPatches = new Float64Array(PETAL_LAYOUT.tileCount * PATCH_CELLS);
  // Normalised read: a different gain and offset for every tile.
  const normalisedPatches = new Float64Array(PETAL_LAYOUT.tileCount * PATCH_CELLS);
  for (let tile = 0; tile < PETAL_LAYOUT.tileCount; tile += 1) {
    const pattern = hypotheses[tile % hypotheses.length];
    const tileGain = 120 + ((tile * 7) % 90);
    const tileOffset = 3 + ((tile * 11) % 40);
    for (let cell = 0; cell < PATCH_CELLS; cell += 1) {
      levelPatches[tile * PATCH_CELLS + cell] = gain * pattern[cell] + offset;
      normalisedPatches[tile * PATCH_CELLS + cell] = tileGain * pattern[cell] + tileOffset;
    }
  }
  // finder levels that describe that gain and offset exactly
  const reference = { lit: new Float64Array(4).fill(offset + gain), dark: new Float64Array(4).fill(offset) };
  // Both reads in turn, twice: each keeps using its own set of templates.
  const sigmas = [0.5, 0]; // the exactly matching blur wins on total error whatever the order
  for (const [name, read] of [
    ["level", () => readTiles(levelPatches, reference, sigmas)],
    ["normalised", () => readTilesNormalised(normalisedPatches, sigmas)],
    ["level again", () => readTiles(levelPatches, reference, sigmas)],
    ["normalised again", () => readTilesNormalised(normalisedPatches, sigmas)],
  ]) {
    const reads = read();
    for (let tile = 0; tile < PETAL_LAYOUT.tileCount; tile += 1) {
      const hypothesis = tile % hypotheses.length;
      assert.ok(reads.error[tile] < 1e-9, `${name}: tile ${tile} error ${reads.error[tile]}`);
      const expected = [hypothesis >> 4, hypothesis & 15];
      assert.deepEqual([reads.light[tile], reads.glyph[tile]], expected, `${name}: tile ${tile}`);
    }
  }
});

test("the level read follows the light across the frame", () => {
  const hypotheses = templatePatches();
  const full = 200;
  // The light falls off to 40% towards the left (or the top) edge, linearly in canvas
  // coordinates; the finder levels at the four corners describe that exactly, so the
  // interpolated levels cancel it. Corners run top left, top right, bottom right, bottom left.
  const fall = (position) => 0.4 + (0.6 * position) / PETAL_LAYOUT.canvas;
  const cases = [
    ["left to right", (x) => fall(x), [fall(0), fall(PETAL_LAYOUT.canvas), fall(PETAL_LAYOUT.canvas), fall(0)]],
    ["top to bottom", (_x, y) => fall(y), [fall(0), fall(0), fall(PETAL_LAYOUT.canvas), fall(PETAL_LAYOUT.canvas)]],
  ];
  for (const [name, gainAt, corners] of cases) {
    const patches = new Float64Array(PETAL_LAYOUT.tileCount * PATCH_CELLS);
    for (let tile = 0; tile < PETAL_LAYOUT.tileCount; tile += 1) {
      const [cx, cy] = tileCenter(tile);
      for (let cell = 0; cell < PATCH_CELLS; cell += 1) {
        patches[tile * PATCH_CELLS + cell] = gainAt(cx, cy) * full * hypotheses[tile % hypotheses.length][cell];
      }
    }
    const reference = { lit: Float64Array.from(corners, (gain) => gain * full), dark: new Float64Array(4) };
    const reads = readTiles(patches, reference, [0]);
    for (let tile = 0; tile < PETAL_LAYOUT.tileCount; tile += 1) {
      const hypothesis = tile % hypotheses.length;
      assert.ok(reads.error[tile] < 1e-9, `${name}: tile ${tile} error ${reads.error[tile]}`);
      const expected = [hypothesis >> 4, hypothesis & 15];
      assert.deepEqual([reads.light[tile], reads.glyph[tile]], expected, `${name}: tile ${tile}`);
    }
  }
});

test("a lane the level read decoded is kept when the normalised read decodes it too", () => {
  const { luma, m, patches } = cleanPatches(5);
  const flat = new Set(flattenFourteenTiles(patches)); // lane P is beyond repair in both reads
  for (let tile = 0; tile < PETAL_LAYOUT.tileCount; tile += 1) {
    if (flat.has(tile) || tile % 3 !== 1) continue;
    for (let cell = 0; cell < PATCH_CELLS; cell += 1) {
      patches[tile * PATCH_CELLS + cell] = 0.55 * patches[tile * PATCH_CELLS + cell] + 20; // dimmed
    }
  }
  const reference = referenceLevels(luma, m);
  const levelK = decodedLane("K", tileWords(readTiles(patches, reference, SIGMAS)));
  const normalisedK = decodedLane("K", tileWords(readTilesNormalised(patches, SIGMAS)));
  // the dimming costs the level read more repairs than the normalised read, which cancels it
  assert.ok(levelK !== null && normalisedK !== null);
  assert.ok(levelK.corrected > normalisedK.corrected, `${levelK.corrected} vs ${normalisedK.corrected}`);
  const lanes = readTileLanes(patches, reference, SIGMAS);
  assert.equal(lanes.p, null);
  assert.deepEqual(lanes.k, levelK, "lane K keeps the level read");
});

test("sampled patches average four bilinear samples around each cell centre", () => {
  const side = 300;
  const rng = new Lcg(11);
  const noise = new PetalLuma(side, side, Uint8Array.from({ length: side * side }, () => rng.byte()));
  const canonical = PETAL_LAYOUT.finderCenters;
  const pose = PetalHomography.fromPoints(canonical, [
    [40, 30],
    [260, 50],
    [250, 270],
    [30, 250],
  ]);
  const patches = samplePatches(noise, Float64Array.from(pose.values));
  const half = PETAL_LAYOUT.glyphBox / 2;
  const cell = PETAL_LAYOUT.glyphBox / PETAL_GLYPHS.templateN;
  for (const tile of [0, 77, 255]) {
    const [cx, cy] = tileCenter(tile);
    for (let v = 0; v < PETAL_GLYPHS.templateN; v += 1) {
      for (let u = 0; u < PETAL_GLYPHS.templateN; u += 1) {
        let sum = 0;
        for (const [ox, oy] of [
          [-0.25, -0.25],
          [0.25, -0.25],
          [-0.25, 0.25],
          [0.25, 0.25],
        ]) {
          const canvasX = cx - half + (u + 0.5) * cell + ox * cell;
          const canvasY = cy - half + (v + 0.5) * cell + oy * cell;
          const [px, py] = pose.apply(canvasX, canvasY);
          sum += noise.sample(px, py);
        }
        const actual = patches[tile * PATCH_CELLS + v * PETAL_GLYPHS.templateN + u];
        assert.ok(Math.abs(actual - sum / 4) < 1e-9, `tile ${tile} cell (${u}, ${v}): ${actual} vs ${sum / 4}`);
      }
    }
  }
});

test("sampled patches are raw luma levels at the tile cells", () => {
  const side = 256;
  const scale = side / PETAL_LAYOUT.canvas;
  const m = [scale, 0, 0, 0, scale, 0, 0, 0, 1];
  const half = PETAL_LAYOUT.glyphBox / 2;
  const cell = PETAL_LAYOUT.glyphBox / PETAL_GLYPHS.templateN;
  // A ramp is reproduced exactly by bilinear sampling (pixel i has its centre at i + 0.5), so
  // each cell holds the ramp at the cell's centre, whatever the offsets of its four samples.
  for (const axis of ["x", "y"]) {
    const ramp = new Uint8Array(side * side);
    for (let y = 0; y < side; y += 1) for (let x = 0; x < side; x += 1) ramp[y * side + x] = axis === "x" ? x : y;
    const out = new Float64Array(PETAL_LAYOUT.tileCount * PATCH_CELLS);
    assert.equal(samplePatches(new PetalLuma(side, side, ramp), m, out), out, "fills and returns the given buffer");
    for (const tile of [0, 1, 17, 128, 255]) {
      const [cx, cy] = tileCenter(tile);
      for (let v = 0; v < PETAL_GLYPHS.templateN; v += 1) {
        for (let u = 0; u < PETAL_GLYPHS.templateN; u += 1) {
          const canvasX = cx - half + (u + 0.5) * cell;
          const canvasY = cy - half + (v + 0.5) * cell;
          const expected = (axis === "x" ? canvasX : canvasY) * scale - 0.5;
          const actual = out[tile * PATCH_CELLS + v * PETAL_GLYPHS.templateN + u];
          const where = `${axis} tile ${tile} cell (${u}, ${v})`;
          assert.ok(Math.abs(actual - expected) < 1e-9, `${where}: ${actual} vs ${expected}`);
        }
      }
    }
  }
  // without a buffer a fresh one is allocated: 256 patches of 64 cells
  assert.equal(samplePatches(new PetalLuma(side, side), m).length, PETAL_LAYOUT.tileCount * PATCH_CELLS);
});

test("a shadowed part of a render decodes through the normalised read", () => {
  const { encoder, luma } = decodeSetup(5);
  const { p: pData, k: kData } = encoder.laneData(5);
  const width = luma.width;
  for (let row = 0; row < luma.height; row += 1) {
    for (let x = Math.floor((width * 7) / 20); x < Math.floor((width * 3) / 5); x += 1) {
      luma.data[row * width + x] = Math.round(luma.data[row * width + x] * 0.3);
    }
  }
  // the finder levels cannot describe a step in the light: the level read loses lane K
  const m = renderHomography();
  const reference = referenceLevels(luma, m);
  const levelWords = tileWords(readTiles(samplePatches(luma, m), reference, SIGMAS));
  assert.equal(decodedLane("K", levelWords), null);
  const decoded = decodePetalFrame(luma);
  assert.deepEqual(decoded.p.data, pData);
  assert.deepEqual(decoded.k.data, kData);
  // a known pose takes the same road
  const at = decodePetalFrameAt(luma, m);
  assert.deepEqual([at.p.data, at.k.data], [pData, kData]);
});

test("lane K alone is enough to accept an orientation", () => {
  const { encoder, luma } = decodeSetup(5);
  const { k: kData } = encoder.laneData(5);
  const size = luma.width;
  const scale = size / PETAL_LAYOUT.canvas;
  const [inner, , outer] = PETAL_LAYOUT.ringRadii;
  // wipe the dotted rings: neither lane D nor the gates that rank orientations survive
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const radius = Math.hypot((x + 0.5) / scale - PETAL_LAYOUT.center, (y + 0.5) / scale - PETAL_LAYOUT.center);
      if (radius > inner - 20 && radius < outer + 20) luma.data[y * size + x] = 0;
    }
  }
  assert.equal(decodePetalFrame(luma).d, null);
  // Flatten fourteen tiles that sit in fourteen different bytes of lane P: that is more than its
  // 13 parity bytes repair, while lane K (45 parity bytes) shrugs it off.
  for (let j = 0; j < 14; j += 1) {
    const [cx, cy] = tileCenter(8 * j);
    for (let y = Math.floor((cy - 14) * scale); y <= Math.ceil((cy + 14) * scale); y += 1) {
      for (let x = Math.floor((cx - 14) * scale); x <= Math.ceil((cx + 14) * scale); x += 1) {
        luma.data[y * size + x] = 128;
      }
    }
  }
  const decoded = decodePetalFrame(luma);
  assert.equal(decoded.p, null);
  assert.equal(decoded.d, null);
  assert.deepEqual(decoded.k.data, kData);
  assert.deepEqual([decoded.rotation, decoded.mirrored, decoded.lanesOk()], [0, false, 1]);
});

// ---------------------------------------------------------------- scan session

/**
 * Minimal camera model for session tests: the rendered canvas is rotated
 * about the image centre, scaled to fit, sampled with 2x2 supersampling and
 * given a little deterministic noise.
 */
function cameraCapture(source, { width, height, rotationDeg = 0, noise = 3, seed = 1 }) {
  const angle = (rotationDeg * Math.PI) / 180;
  const cos = Math.cos(angle);
  const sin = Math.sin(angle);
  const short = Math.min(width, height);
  const fill = Math.min(0.85, (short - 8) / (short * (Math.abs(cos) + Math.abs(sin))));
  const scale = (fill * short) / source.width;
  const rng = new PetalXorshift32(seed);
  const out = new Uint8Array(width * height);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      let sum = 0;
      for (const [ox, oy] of [
        [0.25, 0.25],
        [0.75, 0.25],
        [0.25, 0.75],
        [0.75, 0.75],
      ]) {
        const dx = x + ox - width / 2;
        const dy = y + oy - height / 2;
        const u = (cos * dx + sin * dy) / scale + source.width / 2;
        const v = (-sin * dx + cos * dy) / scale + source.height / 2;
        if (u >= 0 && v >= 0 && u < source.width && v < source.height) sum += source.sample(u, v);
      }
      const value = sum / 4 + noise * (rng.nextByte() / 255 - 0.5);
      out[y * width + x] = Math.min(255, Math.max(0, Math.round(value)));
    }
  }
  return new PetalLuma(width, height, out);
}

test("a session receives a payload from simulated captures", () => {
  const data = payload(500, 3);
  const encoder = new PetalStreamEncoder(data, 2);
  const session = new PetalScanSession();
  let done = null;
  for (let frame = 0; frame < 40; frame += 1) {
    const source = renderLuma(encoder.cells(frame), 512, 2);
    const outcome = session.push(cameraCapture(source, { width: 640, height: 480, rotationDeg: 20, seed: frame + 1 }), frame * 125);
    if (outcome.completed !== null) {
      done = outcome.completed;
      assert.ok(outcome.progress.complete);
      break;
    }
  }
  assert.ok(done !== null, "completed");
  assert.deepEqual(done.payload, data);
  assert.equal(done.meta.kind, 2);
  const stats = session.stats();
  assert.ok(stats.readable > 0 && stats.laneD > 0);
  assert.ok(stats.frames >= stats.located && stats.located >= stats.readable);
});

test("idle sessions forget partial streams", () => {
  const encoder = new PetalStreamEncoder(payload(4000, 3), 1);
  const session = new PetalScanSession({ idleTimeoutMs: 1000 });
  const source = renderLuma(encoder.cells(0), 512, 2);
  session.push(cameraCapture(source, { width: 640, height: 480 }), 0);
  assert.ok(session.progress().rank > 0);
  // a frame much later with nothing readable resets the session first
  const outcome = session.push(new PetalLuma(640, 480), 60000);
  assert.equal(outcome.error, "no_finders");
  assert.equal(outcome.progress.rank, 0);
});

test("located counts codes that were seen but could not be read", () => {
  const encoder = new PetalStreamEncoder(payload(100, 3), 1);
  const frame = renderPetalFrame(encoder.cells(1), { size: 512, supersample: 2 });
  // keep only the four blossoms: finders are located, no lane can be read
  const scale = 512 / 1024;
  for (let y = 0; y < 512; y += 1) {
    for (let x = 0; x < 512; x += 1) {
      const nearFinder = PETAL_LAYOUT.finderCenters.some(([fx, fy]) => Math.hypot(x - fx * scale, y - fy * scale) < 34);
      if (!nearFinder) frame.data.fill(0, (y * 512 + x) * 4, (y * 512 + x) * 4 + 3);
    }
  }
  const session = new PetalScanSession();
  const outcome = session.push(PetalLuma.fromRgba(512, 512, frame.data), 0);
  assert.equal(outcome.error, "no_orientation");
  assert.equal(session.stats().located, 1);
  assert.equal(session.stats().readable, 0);
  // a frame with no code at all is not "located"
  session.push(new PetalLuma(320, 240), 100);
  assert.equal(session.stats().located, 1);
  assert.equal(session.stats().frames, 2);
});

test("unreadable frames do not disturb progress", () => {
  const session = new PetalScanSession();
  const outcome = session.push(new PetalLuma(320, 240), 5);
  assert.equal(outcome.completed, null);
  assert.equal(outcome.lanes, "");
  assert.equal(session.stats().frames, 1);
  assert.equal(session.stats().located, 0);
  assert.equal(PETAL_DEFAULT_SCAN_LIMITS.idleTimeoutMs, 30000);
  assert.equal(PETAL_DEFAULT_SCAN_LIMITS.absoluteTimeoutMs, 180000);
  assert.deepEqual(PETAL_DEFAULT_DECODE_OPTIONS.templateSigmas, [0, 0.5, 0.8, 1.1, 1.5]);
});

// ---------------------------------------------------------------- shared fixtures

const streamFixture = JSON.parse(readFileSync(STREAM_FIXTURE, "utf8"));

test("fixture: constants and layout match", () => {
  const doc = streamFixture;
  assert.equal(doc.fixture_version, 1);
  assert.equal(doc.format, "petal-stream-v1");
  const c = doc.constants;
  assert.deepEqual(
    [c.canvas, c.tile_origin, c.tile_pitch, c.tile_size, c.dot_radius],
    [PETAL_LAYOUT.canvas, PETAL_LAYOUT.tileOrigin, PETAL_LAYOUT.tilePitch, PETAL_LAYOUT.tileSize, PETAL_LAYOUT.dotRadius],
  );
  assert.deepEqual(
    [c.atom_len, c.p_word, c.k_word, c.d_word, c.p_parity, c.k_parity, c.d_parity, c.beacon_interval, c.format_version],
    [
      PETAL_STREAM.atomLen,
      PETAL_LANES.P.wordLen,
      PETAL_LANES.K.wordLen,
      PETAL_LANES.D.wordLen,
      PETAL_LANES.P.parityLen,
      PETAL_LANES.K.parityLen,
      PETAL_LANES.D.parityLen,
      PETAL_STREAM.beaconInterval,
      PETAL_STREAM.formatVersion,
    ],
  );
  const layout = doc.layout;
  assert.deepEqual(layout.mask, PETAL_LAYOUT.mask);
  assert.deepEqual(layout.tiles_col_row, PETAL_LAYOUT.tiles.flat());
  assert.deepEqual(layout.ring_radii, PETAL_LAYOUT.ringRadii);
  assert.deepEqual(layout.ring_slots, PETAL_LAYOUT.ringSlots);
  assert.deepEqual(layout.finder_centers, PETAL_LAYOUT.finderCenters.flat());
  assert.deepEqual(layout.data_slots, dataSlots());
  const roles = slotRoles();
  const of = (kind) => roles.map((role, index) => [role.kind, index]).filter(([k]) => k === kind).map(([, index]) => index);
  assert.deepEqual(layout.gate_slots, of("gate"));
  assert.deepEqual(layout.guard_slots, of("guard"));
});

test("fixture: glyph alphabet, strokes and templates match", () => {
  const glyphs = streamFixture.glyphs;
  assert.equal(glyphs.chars, PETAL_GLYPHS.chars.join(""));
  assert.equal(glyphs.stroke_width, PETAL_GLYPHS.strokeWidth);
  assert.deepEqual(glyphs.strokes, PETAL_GLYPHS.strokes);
  assert.deepEqual(glyphs.templates, PETAL_GLYPHS.templates);
});

test("fixture: checksums, PRNG, mixer and whitening match", () => {
  for (const entry of streamFixture.crc32c) assert.equal(crc32c(fromHex(entry.input_hex)), entry.crc32c);
  const rng = new PetalXorshift32(1);
  assert.deepEqual(
    Array.from({ length: 6 }, () => rng.nextU32()),
    streamFixture.prng.xorshift32_seed1,
  );
  for (const entry of streamFixture.prng.mix32) assert.equal(mix32(entry.in), entry.out);
  for (const lane of ["P", "K", "D"]) assert.equal(hex(laneWhitening(lane)), streamFixture.whitening[lane]);
});

test("fixture: Reed-Solomon vectors encode and correct", () => {
  for (const entry of streamFixture.reed_solomon) {
    const rs = new PetalReedSolomon(entry.nsym);
    const word = rs.encode(fromHex(entry.data_hex));
    assert.equal(hex(word), entry.codeword_hex);
    // damage up to the correction capacity and recover
    const damaged = word.slice();
    for (let i = 0; i < Math.floor(entry.nsym / 2); i += 1) damaged[(i * 3) % word.length] ^= 0x5a;
    rs.decode(damaged, []);
    assert.equal(hex(damaged), entry.codeword_hex);
  }
});

test("fixture: fountain masks and atom ids match", () => {
  for (const entry of streamFixture.fountain_masks) {
    assert.deepEqual(Array.from(maskWords(entry.k, entry.crc, entry.id)), entry.mask);
  }
  const ids = streamFixture.first_atom_ids;
  ids.frames.forEach((frame, index) => assert.equal(firstAtomId(frame), ids.ids[index]));
});

test("fixture: streams encode identically and reassemble", () => {
  for (const stream of streamFixture.streams) {
    const data = fromHex(stream.payload_hex);
    const encoder = new PetalStreamEncoder(data, stream.kind);
    const meta = encoder.meta;
    assert.deepEqual(
      [meta.len, meta.crc, meta.tag, meta.sourceAtoms, encoder.systematicFrames()],
      [stream.len, stream.crc32c, stream.tag, stream.source_atoms, stream.systematic_frames],
      stream.name,
    );
    for (const frame of stream.frames) {
      const lanes = encoder.laneData(frame.frame);
      assert.equal(hex(lanes.p), frame.p_data, `${stream.name} frame ${frame.frame}`);
      assert.equal(hex(lanes.k), frame.k_data, `${stream.name} frame ${frame.frame}`);
      assert.equal(hex(lanes.d), frame.d_data, `${stream.name} frame ${frame.frame}`);
      assert.equal(hex(encodeLane("P", lanes.p)), frame.p_word);
      assert.equal(hex(encodeLane("K", lanes.k)), frame.k_word);
      assert.equal(hex(encodeLane("D", lanes.d)), frame.d_word);
      const words = encoder.words(frame.frame);
      assert.deepEqual([hex(words.p), hex(words.k), hex(words.d)], [frame.p_word, frame.k_word, frame.d_word]);
      const cells = PetalFrameCells.fromWords(fromHex(frame.p_word), fromHex(frame.k_word), fromHex(frame.d_word));
      assert.equal(Array.from(cells.glyph, (g) => g.toString(16)).join(""), frame.glyphs);
      assert.deepEqual(
        Array.from(cells.dots).flatMap((lit, index) => (lit ? [index] : [])),
        frame.lit_dots,
      );
      assert.ok(cells.equals(encoder.cells(frame.frame)));
    }
    // push every fixture frame through decodeLane + assembler
    if (stream.name === "one-pass") {
      const assembler = new PetalStreamAssembler();
      for (const frame of stream.frames) {
        assembler.pushDLane(parseDLane(decodeLane("D", fromHex(frame.d_word))));
        for (const [lane, key] of [
          ["P", "p_word"],
          ["K", "k_word"],
        ]) {
          assembler.pushAtoms(parseAtomLane(lane, decodeLane(lane, fromHex(frame[key]))));
        }
      }
      assert.deepEqual(assembler.takeCompleted().payload, data);
    }
  }
});

// ---------------------------------------------------------------- golden captures

const captureFixture = JSON.parse(readFileSync(CAPTURE_FIXTURE, "utf8"));

function captureLuma(entry) {
  const data = new Uint8Array(inflateSync(Buffer.from(entry.luma_zlib_base64, "base64")));
  return new PetalLuma(entry.width, entry.height, data);
}

test("golden captures decode as recorded", (t) => {
  const assembler = new PetalStreamAssembler();
  for (const capture of captureFixture.captures) {
    const image = captureLuma(capture);
    const started = performance.now();
    const decoded = decodePetalFrame(image);
    const elapsed = performance.now() - started;
    assert.equal(decoded.mirrored, capture.mirrored, `${capture.name}: mirror flag`);
    let lanes = "";
    for (const [letter, lane, expected] of [
      ["P", decoded.p, capture.p_data],
      ["K", decoded.k, capture.k_data],
      ["D", decoded.d, capture.d_data],
    ]) {
      if (lane !== null) {
        assert.equal(hex(lane.data), expected, `${capture.name}: lane ${letter} data`);
        lanes += letter;
      } else {
        assert.ok(!capture.must_decode.includes(letter), `${capture.name}: required lane ${letter} was not decoded`);
      }
    }
    // the decoder follows the reference step by step, so under V8 it reads
    // exactly the lanes the reference read
    assert.equal(lanes, capture.reference_decoded, `${capture.name}: lanes differ from the reference`);
    t.diagnostic(`${capture.name} ${capture.width}x${capture.height}: lanes ${lanes} in ${elapsed.toFixed(1)} ms`);
    decoded.feed(assembler);
  }
  // captures of different frames of the same stream accumulate in one assembler
  assert.ok(assembler.progress().atomsReceived > 10);
});

/** The captures of bad lighting: only the normalised tile read gets their tile lanes. */
const LIGHTING_CAPTURES = ["overexposed-540p", "veiled-720p", "shadow-band-540p"];

function namedCapture(name) {
  const capture = captureFixture.captures.find((entry) => entry.name === name);
  assert.ok(capture !== undefined, `capture ${name} is part of the fixture`);
  return capture;
}

/** The pose a capture was read under, as nine coefficients, with its reference levels and raw patches. */
function capturePatches(capture) {
  const image = captureLuma(capture);
  const m = Float64Array.from(decodePetalFrame(image).homography.values);
  return { image, m, reference: referenceLevels(image, m), patches: samplePatches(image, m) };
}

test("the fixture holds the lighting captures", () => {
  assert.ok(captureFixture.captures.length >= 9, "six camera captures and the three lighting ones");
  for (const name of LIGHTING_CAPTURES) namedCapture(name);
  // every capture records the lanes the reference decoder read; they cover the required ones
  for (const capture of captureFixture.captures) {
    for (const letter of capture.must_decode) assert.ok(capture.reference_decoded.includes(letter), capture.name);
  }
});

test("the lighting captures need the normalised read", () => {
  for (const name of LIGHTING_CAPTURES) {
    const capture = namedCapture(name);
    const { reference, patches } = capturePatches(capture);
    const words = tileWords(readTiles(patches, reference, SIGMAS));
    const levelLanes = ["P", "K"].filter((lane) => decodedLane(lane, words) !== null);
    const required = [...capture.must_decode].filter((lane) => lane !== "D");
    assert.ok(
      required.some((lane) => !levelLanes.includes(lane)),
      `${name}: the level read alone already reads ${levelLanes.join("")}`,
    );
    // ... and the combined read delivers every required tile lane with the recorded data
    const lanes = readTileLanes(patches, reference, SIGMAS);
    for (const lane of required) {
      assert.equal(hex(lanes[lane.toLowerCase()].data), capture[`${lane.toLowerCase()}_data`], `${name}: lane ${lane}`);
    }
  }
});

test("the normalised read only fills in the lanes the level read missed", () => {
  const capture = namedCapture("veiled-720p");
  const { reference, patches } = capturePatches(capture);
  const levelWords = tileWords(readTiles(patches, reference, SIGMAS));
  const normalisedWords = tileWords(readTilesNormalised(patches, SIGMAS));
  const [levelP, levelK] = ["P", "K"].map((lane) => decodedLane(lane, levelWords));
  const [normalisedP, normalisedK] = ["P", "K"].map((lane) => decodedLane(lane, normalisedWords));
  // this capture separates the reads: the level read has lane P (after repairs) but not K, and
  // the normalised read has both, with a different repair count for P
  assert.ok(levelP !== null && levelK === null && normalisedP !== null && normalisedK !== null);
  assert.notDeepEqual(levelP, normalisedP);
  const lanes = readTileLanes(patches, reference, SIGMAS);
  assert.deepEqual(lanes.p, levelP, "lane P keeps the level read");
  assert.deepEqual(lanes.k, normalisedK, "lane K comes from the normalised read");
  assert.equal(hex(lanes.p.data), capture.p_data);
  assert.equal(hex(lanes.k.data), capture.k_data);
});

test("a scan session reads the recorded lanes of every golden capture", () => {
  for (const capture of captureFixture.captures) {
    const session = new PetalScanSession();
    const outcome = session.push(captureLuma(capture), 0);
    assert.equal(outcome.error, null, capture.name);
    assert.equal(outcome.lanes, capture.reference_decoded, capture.name);
    const stats = session.stats();
    const counted = ["P", "K", "D"].map((lane) => (capture.reference_decoded.includes(lane) ? 1 : 0));
    assert.deepEqual([stats.laneP, stats.laneK, stats.laneD], counted, capture.name);
  }
});

test("negative captures are rejected", () => {
  for (const negative of captureFixture.negatives) {
    assert.throws(() => decodePetalFrame(captureLuma(negative)), PetalError, negative.name);
  }
});

test("recorded lane data is a valid codeword of its stream", () => {
  const encoder = new PetalStreamEncoder(fromHex(captureFixture.payload_hex), captureFixture.payload_kind);
  for (const capture of captureFixture.captures) {
    const data = fromHex(capture.p_data);
    assert.deepEqual(decodeLane("P", encodeLane("P", data)), data);
    const expected = encoder.laneData(capture.frame);
    assert.deepEqual([hex(expected.p), hex(expected.k), hex(expected.d)], [capture.p_data, capture.k_data, capture.d_data]);
  }
});

// ---------------------------------------------------------------- browser glue

function recordingContext(canvas = { width: 512, height: 512 }) {
  const calls = [];
  const context = { canvas, calls };
  for (const name of ["save", "restore", "translate", "scale", "fillRect", "beginPath", "moveTo", "lineTo", "arc", "arcTo", "closePath", "rect", "clip", "fill", "stroke"]) {
    context[name] = (...args) => calls.push([name, ...args]);
  }
  return context;
}

test("petalScanSize keeps the long side within the limit", () => {
  assert.deepEqual(petalScanSize(1920, 1080), { width: 1280, height: 720 });
  assert.deepEqual(petalScanSize(1080, 1920), { width: 720, height: 1280 });
  assert.deepEqual(petalScanSize(640, 480), { width: 640, height: 480 });
  assert.deepEqual(petalScanSize(4000, 3000, 1000), { width: 1000, height: 750 });
  assert.deepEqual(petalScanSize(5000, 1, 1280), { width: 1280, height: 1 });
  assert.throws(() => petalScanSize(0, 10), TypeError);
});

test("petalPlayerFrame advances at the frame rate and wraps the counter", () => {
  assert.equal(petalPlayerFrame(0, 0, 10), 0);
  assert.equal(petalPlayerFrame(0, 99.9, 10), 0);
  assert.equal(petalPlayerFrame(0, 100, 10), 1);
  assert.equal(petalPlayerFrame(5, 1000, 12), 17);
  assert.equal(petalPlayerFrame(65535, 100, 10), 0);
  assert.equal(petalPlayerFrame(3, -50, 10), 3);
  assert.throws(() => petalPlayerFrame(0, 0, 0), TypeError);
});

test("drawPetalFrame paints the draw list with round strokes", () => {
  const encoder = new PetalStreamEncoder(payload(300, 8), 1);
  const cells = encoder.cells(3);
  const list = petalDrawList(cells);
  const context = recordingContext();
  drawPetalFrame(context, list);
  const count = (name) => context.calls.filter((call) => call[0] === name).length;
  assert.deepEqual(context.calls.find((call) => call[0] === "scale"), ["scale", 0.5, 0.5]);
  assert.equal(count("arc"), 4 * 11 + list.dots.length);
  assert.equal(count("arcTo"), 4 * cells.light.reduce((a, b) => a + b, 0));
  assert.equal(count("rect"), 256);
  assert.equal(count("clip"), 1);
  assert.equal(count("stroke"), 2);
  assert.equal(count("save"), count("restore"));
  assert.equal(context.lineCap, "round");
  assert.equal(context.lineJoin, "round");
  assert.equal(context.lineWidth, list.strokeWidth);
  const strokes = list.tiles.reduce((sum, tile) => sum + tile.strokes.length, 0);
  const lightTiles = cells.light.reduce((a, b) => a + b, 0);
  assert.equal(count("moveTo"), 4 * 11 + list.dots.length + lightTiles + strokes);
});

test("the stream player draws frames on animation ticks", () => {
  const encoder = new PetalStreamEncoder(payload(300, 9), 1);
  const context = recordingContext();
  const queue = [];
  const shown = [];
  const player = new PetalStreamPlayer({
    encoder,
    context,
    fps: 10,
    requestAnimationFrame: (callback) => queue.push(callback),
    cancelAnimationFrame: () => {},
    onFrame: (frame) => shown.push(frame),
  });
  player.start();
  assert.ok(player.playing);
  for (const time of [1000, 1050, 1100, 1350, 1360]) queue.shift()(time);
  assert.deepEqual(shown, [0, 1, 3]);
  assert.equal(player.frame, 3);
  player.stop();
  assert.ok(!player.playing);
  const pendingBefore = queue.length;
  queue.shift()(5000);
  assert.deepEqual(shown, [0, 1, 3]);
  assert.equal(queue.length, pendingBefore - 1);
  player.drawFrame(70000);
  assert.equal(player.frame, 70000 % 65536);
  assert.throws(() => new PetalStreamPlayer({ encoder: {}, context }), TypeError);
});

test("the camera scanner reads a stream from video frames", async () => {
  const data = payload(260, 12);
  const encoder = new PetalStreamEncoder(data, 4);
  const frames = [];
  for (let frame = 0; frame < 6; frame += 1) frames.push(renderPetalFrame(encoder.cells(frame), { size: 320, supersample: 2 }));
  let current = 0;
  const callbacks = [];
  const video = {
    videoWidth: 320,
    videoHeight: 320,
    paused: false,
    requestVideoFrameCallback: (callback) => callbacks.push(callback),
    cancelVideoFrameCallback: () => {},
  };
  const drawn = [];
  const context = {
    drawImage: (_source, x, y, width, height) => drawn.push([x, y, width, height]),
    getImageData: (_x, _y, width, height) => ({ width, height, data: frames[current % frames.length].data }),
  };
  const canvas = { width: 0, height: 0, getContext: (kind, settings) => (kind === "2d" && settings.willReadFrequently ? context : null) };
  let completed = null;
  const outcomes = [];
  let clock = 0;
  const scanner = new PetalCameraScanner({
    video,
    createCanvas: (width, height) => Object.assign(canvas, { width, height }),
    onProgress: (outcome) => outcomes.push(outcome),
    onComplete: (result) => {
      completed = result;
    },
    now: () => clock,
  });
  await scanner.start();
  while (callbacks.length > 0 && completed === null) {
    callbacks.shift()(clock, {});
    current += 1;
    clock += 100;
  }
  assert.ok(completed !== null);
  assert.deepEqual(completed.payload, data);
  assert.equal(completed.meta.kind, 4);
  assert.ok(!scanner.scanning);
  assert.equal(callbacks.length, 0, "a completed scan stops requesting frames");
  assert.deepEqual(drawn[0], [0, 0, 320, 320]);
  assert.ok(outcomes.every((outcome) => outcome.error === null));
  assert.ok(scanner.session.stats().readable >= 1);
});

test("the camera scanner downsizes large frames and tolerates frames without data", async () => {
  const queue = [];
  const video = { videoWidth: 0, videoHeight: 0, paused: false };
  const sizes = [];
  const context = {
    drawImage: () => {},
    getImageData: (_x, _y, width, height) => {
      sizes.push([width, height]);
      return { width, height, data: new Uint8ClampedArray(width * height * 4) };
    },
  };
  const scanner = new PetalCameraScanner({
    video,
    createCanvas: (width, height) => ({ width, height, getContext: () => context }),
    requestAnimationFrame: (callback) => queue.push(callback),
    cancelAnimationFrame: () => {},
    now: () => 0,
  });
  await scanner.start();
  queue.shift()(0);
  assert.deepEqual(sizes, [], "no frame before the video has dimensions");
  video.videoWidth = 2560;
  video.videoHeight = 1440;
  queue.shift()(16);
  assert.deepEqual(sizes, [[1280, 720]]);
  assert.equal(scanner.session.stats().frames, 1);
  scanner.stop();
  assert.ok(!scanner.scanning);
  assert.throws(() => new PetalCameraScanner({}), TypeError);
});

test("stopping the camera scanner while playback starts wins", async () => {
  let resolvePlay = null;
  let plays = 0;
  const requested = [];
  const video = {
    videoWidth: 64,
    videoHeight: 64,
    paused: true,
    srcObject: null,
    play: () => {
      plays += 1;
      return new Promise((resolve) => {
        resolvePlay = resolve;
      });
    },
    requestVideoFrameCallback: (callback) => requested.push(callback),
    cancelVideoFrameCallback: () => {},
  };
  const stream = { id: "camera" };
  const scanner = new PetalCameraScanner({ video, stream, createCanvas: () => ({ width: 0, height: 0, getContext: () => null }) });
  const first = scanner.start();
  assert.equal(scanner.start(), first, "a pending start is shared");
  assert.equal(video.srcObject, stream);
  scanner.stop();
  resolvePlay();
  await first;
  assert.ok(!scanner.scanning);
  assert.equal(requested.length, 0);
  const second = scanner.start();
  assert.notEqual(second, first);
  resolvePlay();
  await second;
  assert.ok(scanner.scanning);
  assert.equal(requested.length, 1);
  assert.equal(plays, 2);
  scanner.stop();
  assert.ok(!scanner.scanning);
});

test("the module bundles for browsers without Node built-ins", async () => {
  const result = await build({
    stdin: { contents: 'export * from "./src/petal.js";', resolveDir: fileURLToPath(new URL("..", import.meta.url)) },
    bundle: true,
    platform: "browser",
    format: "esm",
    target: "es2020",
    write: false,
    metafile: true,
  });
  const inputs = Object.keys(result.metafile.inputs);
  assert.deepEqual(findForbiddenBrowserInputs(inputs), []);
  assert.ok(inputs.every((input) => input === "<stdin>" || /src[/\\]petal/u.test(input)), inputs.join(", "));
  const text = result.outputFiles[0].text;
  assert.doesNotMatch(text, /\bBuffer\b|\brequire\(|node:/u);
  const browser = await import(`data:text/javascript;base64,${Buffer.from(text).toString("base64")}`);
  assert.equal(browser.crc32c(new TextEncoder().encode("123456789")), 0xe3069283);
  assert.equal(typeof browser.PetalCameraScanner, "function");
});

test("the declarations type-check a strict browser consumer", () => {
  const declaration = fileURLToPath(new URL("../petal.js", import.meta.url));
  const work = mkdtempSync(path.join(os.tmpdir(), "iroha-petal-types-"));
  try {
    const consumer = path.join(work, "consumer.mts");
    writeFileSync(
      consumer,
      [
        "import {",
        "  PETAL_LANES, PetalCameraScanner, PetalDecodedFrame, PetalError, PetalLuma, PetalScanSession, PetalStreamEncoder,",
        "  PetalStreamPlayer, decodePetalFrame, drawPetalFrame, encodeLane, petalDrawList, renderPetalFrame,",
        "  type PetalCompleted, type PetalDecodeErrorCode, type PetalScanOutcome,",
        `} from ${JSON.stringify(declaration)};`,
        "declare const canvas: HTMLCanvasElement;",
        "declare const video: HTMLVideoElement;",
        "declare const stream: MediaStream;",
        "const context = canvas.getContext(\"2d\");",
        "if (context === null) throw new Error(\"no 2d context\");",
        "const encoder = new PetalStreamEncoder(new Uint8Array([1, 2, 3]), 2);",
        "const player = new PetalStreamPlayer({ encoder, context, fps: 12, cancelAnimationFrame: (id: number) => cancelAnimationFrame(id) });",
        "player.start();",
        "const scanner = new PetalCameraScanner({",
        "  video, stream, maxSide: 960, createCanvas: (width, height) => new OffscreenCanvas(width, height),",
        "  onProgress: (outcome: PetalScanOutcome) => void outcome.progress.rank,",
        "  onComplete: (completed: PetalCompleted) => void completed.payload.byteLength,",
        "});",
        "void scanner.start();",
        "const image = renderPetalFrame(encoder.cells(0), { size: 256, supersample: 2 });",
        "const luma = PetalLuma.fromImageData(new ImageData(image.data, image.width, image.height));",
        "if (luma !== null) {",
        "  try {",
        "    const lanes: number = decodePetalFrame(luma, { tryMirrored: false }).lanesOk();",
        "    void lanes;",
        "  } catch (error) {",
        "    if (error instanceof PetalError) { const code: string = error.code; void code; }",
        "  }",
        "}",
        "drawPetalFrame(context, petalDrawList(encoder.cells(1)), { size: 512 });",
        "const session = new PetalScanSession({ idleTimeoutMs: 10_000, decode: { maxPixels: 2_000_000 } });",
        "const outcome = session.push(new PetalLuma(64, 64), performance.now());",
        "const reason: PetalDecodeErrorCode | null = outcome.error;",
        "const kLen: number = PETAL_LANES.K.wordLen;",
        "void reason; void kLen; void encodeLane(\"P\", new Uint8Array(19));",
        "// @ts-expect-error lanes are named P, K or D",
        "encodeLane(\"X\", new Uint8Array(19));",
        "// @ts-expect-error decoded frames come from the decoder only",
        "void new PetalDecodedFrame();",
      ].join("\n"),
    );
    const program = ts.createProgram([consumer], {
      strict: true,
      exactOptionalPropertyTypes: true,
      noUncheckedIndexedAccess: true,
      skipLibCheck: false,
      target: ts.ScriptTarget.ES2022,
      module: ts.ModuleKind.NodeNext,
      moduleResolution: ts.ModuleResolutionKind.NodeNext,
      lib: ["lib.es2022.d.ts", "lib.dom.d.ts"],
      types: [],
      noEmit: true,
    });
    const diagnostics = ts
      .getPreEmitDiagnostics(program)
      .map((diagnostic) => ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n"));
    assert.deepEqual(diagnostics, []);
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
});

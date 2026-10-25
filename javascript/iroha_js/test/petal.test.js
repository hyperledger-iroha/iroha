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
  PETAL_LANES,
  PETAL_LAYOUT,
  PETAL_PALETTE,
  PETAL_STREAM,
  PETAL_TRACK_WINDOW_MS,
  PetalCameraScanner,
  PetalDecodedFrame,
  PetalError,
  PetalFieldEvidence,
  PetalFountainDecoder,
  PetalFrameCells,
  PetalHomography,
  PetalLuma,
  PetalPose,
  PetalReedSolomon,
  PetalScanSession,
  PetalStreamAssembler,
  PetalStreamEncoder,
  PetalStreamPlayer,
  PetalXorshift32,
  atomsInFrame,
  bandPhase,
  bandRadius,
  blossoms,
  cellAngle,
  cellBandSlot,
  cellCenter,
  cellLight,
  crc32c,
  dataSlots,
  decodeLane,
  decodeLaneCounted,
  decodePetalFrame,
  decodePetalFrameAt,
  drawPetalFrame,
  encodeAtom,
  encodeLane,
  fieldEvidence,
  fieldLane,
  finderLit,
  firstAtomId,
  followFinder,
  inMask,
  isBeaconFrame,
  labelComponents,
  laneFirstId,
  laneLetter,
  laneWhitening,
  locate,
  locateCandidates,
  maskWords,
  mergeFields,
  mix32,
  observedCells,
  parseAtomLane,
  parseDLane,
  petalDrawList,
  petalLit,
  petalPlayerFrame,
  petalScanSize,
  renderPetalFrame,
  sectorCells,
  selectQuad,
  selectTriple,
  slotCenter,
  slotRoles,
  splitPayload,
  splitSlot,
  trackPetalFrame,
} from "../src/petal.js";
import {
  decodeWithErasures,
  localLevels,
  neighbourhoods,
  readFields,
  readSectors,
  referenceLevels,
  twoMeans,
} from "../src/petal/decode.js";
import { completeTriple, strongFinders } from "../src/petal/locate.js";

const STREAM_FIXTURE = new URL("../../../fixtures/petal/petal_stream_v2.json", import.meta.url);
const CAPTURE_FIXTURE = new URL("../../../fixtures/petal/petal_captures_v2.json", import.meta.url);

/** Lane D, then the four field lanes. */
const ALL_LANES = Object.freeze(["D", "F0", "F1", "F2", "F3"]);
const NO_SKIP = Object.freeze([false, false, false, false]);
const TAU = 2 * Math.PI;

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

/** The bytes of `lane` among lane data or codewords `{d, fields}`. */
function laneBytes(words, lane) {
  return lane === "D" ? words.d : words.fields[Number(lane[1])];
}

function feedFrame(assembler, encoder, frame, lanes) {
  const words = encoder.words(frame);
  for (const lane of lanes) {
    const data = decodeLane(lane, laneBytes(words, lane));
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

/** One flag per field cell, cell 0 in the most significant bit of byte 0. */
function cellFlags(bytes) {
  return Array.from({ length: PETAL_LAYOUT.fieldCells }, (_, cell) => (bytes[cell >> 3] >> (7 - (cell & 7))) & 1);
}

/** Field data of the lane tests: the reference's `field_data(seed)`. */
function fieldData(seed) {
  return Uint8Array.from({ length: PETAL_LANES.F0.dataLen }, (_, b) => ((b * 37) & 0xff) ^ seed);
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

// ---------------------------------------------------------------- layout

test("the mask is mirror-symmetric and shows which way is up", () => {
  assert.equal(PETAL_LAYOUT.mask.length, 20);
  for (const row of PETAL_LAYOUT.mask) {
    assert.equal(row.length, 20);
    for (let col = 0; col < 10; col += 1) assert.equal(row[col], row[19 - col], `row ${row} not mirrored`);
  }
  assert.notEqual(PETAL_LAYOUT.mask[0], PETAL_LAYOUT.mask[19]);
});

test("bands hold every cell and split into equal sectors", () => {
  assert.equal(PETAL_LAYOUT.bandOffsets[PETAL_LAYOUT.bandCount], PETAL_LAYOUT.fieldCells);
  assert.equal(PETAL_LAYOUT.bandSlots.reduce((a, b) => a + b, 0), 3376);
  const seen = new Uint8Array(PETAL_LAYOUT.fieldCells);
  for (let sector = 0; sector < PETAL_LAYOUT.sectors; sector += 1) {
    const cells = sectorCells(sector);
    assert.equal(cells.length, PETAL_LAYOUT.sectorCells);
    for (const cell of cells) {
      assert.equal(seen[cell], 0, `cell ${cell} in two sectors`);
      seen[cell] = 1;
      // a cell sits in its own quarter turn (odd bands lean half a slot forward)
      const [band, slot] = cellBandSlot(cell);
      const quarter = PETAL_LAYOUT.bandSlots[band] / 4;
      assert.equal(Math.floor(slot / quarter), sector);
      const turn = (cellAngle(cell) / TAU) * 4;
      assert.ok(turn > sector - 1e-9 && turn < sector + 1, `cell ${cell} turn ${turn}`);
    }
  }
  assert.ok(seen.every((flag) => flag === 1));
  assert.throws(() => sectorCells(4), RangeError);
  assert.throws(() => cellCenter(3376), RangeError);
});

test("band slots follow the pitch rule", () => {
  PETAL_LAYOUT.bandSlots.forEach((slots, band) => {
    const raw = (TAU * bandRadius(band)) / 50;
    assert.equal(slots, 4 * Math.max(Math.round(raw), 1), `band ${band}`);
    assert.equal(bandRadius(band), 16 + 11 * band);
    assert.equal(bandPhase(band), band % 2 === 1 ? 0.5 : 0);
  });
});

test("the field maps onto itself under a quarter turn", () => {
  for (let cell = 0; cell < PETAL_LAYOUT.fieldCells; cell += 1) {
    const [band, slot] = cellBandSlot(cell);
    const n = PETAL_LAYOUT.bandSlots[band];
    const turned = PETAL_LAYOUT.bandOffsets[band] + ((slot + n / 4) % n);
    const [x, y] = cellCenter(cell);
    const [tx, ty] = cellCenter(turned);
    // a quarter turn clockwise on screen about the centre
    assert.ok(Math.abs(tx - (512 - (y - 512))) < 1e-9 && Math.abs(ty - (512 + (x - 512))) < 1e-9, `cell ${cell}`);
  }
});

test("petals stay clear of their neighbours and the rings", () => {
  const outer = bandRadius(PETAL_LAYOUT.bandCount - 1) + PETAL_LAYOUT.petalHalfLength;
  assert.ok(outer + 10 < PETAL_LAYOUT.ringRadii[0] - PETAL_LAYOUT.dotRadius);
  const offsets = PETAL_LAYOUT.bandOffsets;
  for (let cell = 0; cell < PETAL_LAYOUT.fieldCells; cell += 1) {
    const [x, y] = cellCenter(cell);
    const [band] = cellBandSlot(cell);
    const last = offsets[Math.min(band + 2, PETAL_LAYOUT.bandCount)];
    for (let other = offsets[Math.max(band - 1, 0)]; other < last; other += 1) {
      if (other === cell) continue;
      const [ox, oy] = cellCenter(other);
      const distance = Math.hypot(x - ox, y - oy);
      assert.ok(distance > 2 * PETAL_LAYOUT.petalRadius, `cells ${cell} and ${other} are ${distance} apart`);
    }
  }
});

test("the light cells draw the character", () => {
  let light = 0;
  for (let cell = 0; cell < PETAL_LAYOUT.fieldCells; cell += 1) if (cellLight(cell)) light += 1;
  assert.ok(light >= 1200 && light < 2400, `${light} light cells`);
  assert.ok(inMask(512, 190), "the top bar");
  assert.ok(!inMask(512, 350), "the gap under the second bar");
  assert.ok(!inMask(181.9, 300) && !inMask(300, 842), "outside the lattice");
});

test("ring slots provide exactly the lane D capacity", () => {
  const roles = slotRoles();
  assert.equal(roles.length, 296);
  assert.equal(roles.filter((role) => role.kind === "data").length, 240);
  assert.equal(roles.filter((role) => role.kind === "gate").length, 4 + 5 + 7);
  assert.equal(roles.filter((role) => role.kind === "guard").length, 18);
  assert.equal(dataSlots().length, 240);
  assert.equal(roles.filter((role) => role.kind === "spare").length, 296 - 240 - 16 - 18);
  assert.deepEqual(splitSlot(0), [0, 0]);
  assert.deepEqual(splitSlot(88), [1, 0]);
  assert.deepEqual(splitSlot(295), [2, 107]);
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

test("finders and rings do not overlap", () => {
  const outermost = PETAL_LAYOUT.ringRadii[2] + PETAL_LAYOUT.dotRadius;
  for (const [x, y] of PETAL_LAYOUT.finderCenters) {
    const distance = Math.hypot(x - 512, y - 512);
    assert.ok(distance - PETAL_LAYOUT.finderOuter > outermost + 20);
  }
});

test("slot centres are the single-precision reference values", () => {
  assert.deepEqual(slotCenter(0, 0), [917, 512]);
  // f32 bit patterns of slot (0, 1) from the reference: 0x4464FDF6, 0x4407391D
  const view = new DataView(new ArrayBuffer(4));
  const [x, y] = slotCenter(0, 1);
  view.setFloat32(0, x);
  assert.equal(view.getUint32(0), 0x4464fdf6);
  view.setFloat32(0, y);
  assert.equal(view.getUint32(0), 0x4407391d);
  assert.ok(finderLit(0, 0) && finderLit(0, -34) && !finderLit(0, -59) && !finderLit(59, 59));
});

test("a petal is lit at its centre and notched at its tip", () => {
  assert.ok(petalLit(0, 0, 0.3));
  const tip = PETAL_LAYOUT.petalNotchDistance;
  assert.ok(!petalLit(tip * Math.cos(0.3), tip * Math.sin(0.3), 0.3));
  assert.ok(!petalLit(0, 10, 0));
  // the ellipse is longer along its outward direction than across it
  assert.ok(petalLit(-5, 0, 0) && !petalLit(0, -5, 0));
});

// ---------------------------------------------------------------- lanes

test("lane sizes are consistent", () => {
  assert.deepEqual(Object.keys(PETAL_LANES), ALL_LANES);
  assert.deepEqual([PETAL_LANES.D.wordLen, PETAL_LANES.D.parityLen, PETAL_LANES.D.dataLen], [30, 11, 19]);
  for (const lane of ALL_LANES) {
    if (lane !== "D") {
      assert.deepEqual([PETAL_LANES[lane].wordLen, PETAL_LANES[lane].parityLen, PETAL_LANES[lane].dataLen], [105, 22, 83]);
    }
    assert.equal(PETAL_LANES[lane].dataLen, PETAL_STREAM.laneHeaderLen + PETAL_LANES[lane].atoms * PETAL_STREAM.atomLen);
  }
  assert.equal(PETAL_STREAM.spareSectorCells, 4);
  assert.equal(PETAL_STREAM.atomsPerFrame, 21);
  assert.deepEqual([0, 1, 2, 3].map(fieldLane), ["F0", "F1", "F2", "F3"]);
  assert.equal(ALL_LANES.map(laneLetter).join(""), "D0123");
  assert.throws(() => fieldLane(4), RangeError);
});

test("whitening is deterministic and balanced", () => {
  for (const lane of ALL_LANES) {
    const a = laneWhitening(lane);
    assert.deepEqual(a, laneWhitening(lane));
    let ones = 0;
    for (const byte of a) for (let bit = 0; bit < 8; bit += 1) ones += (byte >> bit) & 1;
    const bits = a.length * 8;
    assert.ok(ones > Math.floor((bits * 38) / 100) && ones < Math.floor((bits * 62) / 100), `${lane} ones ${ones}/${bits}`);
  }
  assert.notDeepEqual(laneWhitening("F0"), laneWhitening("F1"));
});

test("lanes roundtrip through cells", () => {
  const dData = Uint8Array.from({ length: 19 }, (_, b) => b ^ 0xa5);
  const d = encodeLane("D", dData);
  const fields = [0, 1, 2, 3].map((sector) => encodeLane(fieldLane(sector), fieldData(sector)));
  const cells = PetalFrameCells.fromWords(d, fields);
  assert.deepEqual(cells.dWord(), d);
  assert.deepEqual(decodeLane("D", cells.dWord()), dData);
  fields.forEach((field, sector) => {
    assert.deepEqual(cells.fieldWord(sector), field);
    assert.deepEqual(decodeLane(fieldLane(sector), cells.fieldWord(sector)), fieldData(sector));
  });
  // the spare cells at the end of every sector stay dark
  for (let sector = 0; sector < 4; sector += 1) {
    for (const cell of sectorCells(sector).slice(840)) assert.equal(cells.petals[cell], 0);
  }
  assert.throws(() => encodeLane("F0", new Uint8Array(82)), RangeError);
  assert.throws(() => encodeLane("P", dData), TypeError);
  assertPetalError(() => decodeLane("F0", new Uint8Array(104)), "rs_invalid_shape");
  assert.throws(() => PetalFrameCells.fromWords(d, fields.slice(0, 3)), TypeError);
  assert.throws(() => PetalFrameCells.fromWords(d, [...fields.slice(0, 3), new Uint8Array(104)]), RangeError);
  assert.throws(() => new PetalFrameCells(new Uint8Array(295)), RangeError);
  assert.ok(cells.equals(PetalFrameCells.fromWords(d, fields)));
  assert.ok(!cells.equals(new PetalFrameCells()));
});

test("all-zero data still lights about half the petals", () => {
  const zero = new Uint8Array(83);
  const cells = PetalFrameCells.fromWords(
    encodeLane("D", new Uint8Array(19)),
    [0, 1, 2, 3].map((sector) => encodeLane(fieldLane(sector), zero)),
  );
  const lit = cells.petals.reduce((sum, value) => sum + value, 0);
  assert.ok(lit >= 1300 && lit <= 2050, `${lit} lit petals`);
});

test("gate dots are always lit and guards dark", () => {
  const field = new Uint8Array(105).fill(0xff);
  const cells = PetalFrameCells.fromWords(new Uint8Array(30).fill(0xff), [field, field, field, field]);
  slotRoles().forEach((role, slot) => {
    if (role.kind === "gate" || role.kind === "data") assert.equal(cells.dots[slot], 1);
    else assert.equal(cells.dots[slot], 0);
  });
});

test("counted decoding reports the rewritten positions", () => {
  const data = fieldData(9);
  const clean = encodeLane("F2", data);
  const exact = decodeLaneCounted("F2", clean);
  assert.deepEqual([Array.from(exact.data), exact.corrected], [Array.from(data), 0]);
  const damaged = clean.slice();
  for (const position of [0, 7, 19, 31, 60, 104]) damaged[position] ^= 0xc3;
  const counted = decodeLaneCounted("F2", damaged, []);
  assert.deepEqual([Array.from(counted.data), counted.corrected], [Array.from(data), 6]);
  assert.deepEqual(decodeLane("F2", damaged), data);
  assertPetalError(() => decodeLaneCounted("F0", new Uint8Array(105).fill(0x55), []), "rs_uncorrectable");
});

test("a word decoded under another sector is refused", () => {
  const word = encodeLane("F0", fieldData(1));
  for (const lane of ["F1", "F2", "F3"]) assertPetalError(() => decodeLane(lane, word), "rs_uncorrectable");
});

test("field lane decoding survives burst damage", () => {
  const data = fieldData(4);
  const word = encodeLane("F3", data);
  for (let index = 40; index < 51; index += 1) word[index] ^= 0x5a;
  assert.deepEqual(decodeLane("F3", word), data);
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
  assert.equal(atomsInFrame(0), 20);
  assert.equal(atomsInFrame(1), 21);
  assert.equal(laneFirstId("F0", 4), firstAtomId(4));
  assert.equal(laneFirstId("F3", 5), firstAtomId(5) + 15);
  assert.equal(laneFirstId("D", 5), firstAtomId(5) + 20);
  const encoder = new PetalStreamEncoder(payload(300, 1), 1);
  assert.equal(parseAtomLane("F2", encoder.laneData(4).fields[2]).firstId, firstAtomId(4) + 10);
  assert.equal(parseDLane(encoder.laneData(5).d).packet.firstId, firstAtomId(5) + 20);
  // the frame counter wraps on a beacon frame, so ids repeat cleanly
  assert.ok(isBeaconFrame(0) && 65536 % PETAL_STREAM.beaconInterval === 0);
});

test("a clean stream completes after one systematic pass", () => {
  const data = payload(1000, 21);
  const encoder = new PetalStreamEncoder(data, 2);
  const assembler = new PetalStreamAssembler();
  for (let frame = 0; frame < encoder.systematicFrames(); frame += 1) feedFrame(assembler, encoder, frame, ALL_LANES);
  const done = assembler.takeCompleted();
  assert.deepEqual(done.payload, data);
  assert.equal(done.meta.kind, 2);
  assert.ok(assembler.progress().complete);
});

test("any single lane is enough given a beacon", () => {
  const data = payload(400, 22);
  const encoder = new PetalStreamEncoder(data, 1);
  for (const lane of ALL_LANES) {
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
  // frames 1..: no beacon among them until frame 4
  for (let frame = 1; frame < 4; frame += 1) feedFrame(assembler, encoder, frame, ALL_LANES);
  assert.equal(assembler.progress().meta, null);
  for (let frame = 4; frame < encoder.systematicFrames() + 4; frame += 1) feedFrame(assembler, encoder, frame, ALL_LANES);
  assert.deepEqual(assembler.takeCompleted().payload, data);
});

test("joining mid-stream and losing frames still completes", () => {
  const data = payload(2500, 24);
  const encoder = new PetalStreamEncoder(data, 3);
  const assembler = new PetalStreamAssembler();
  const rng = new PetalXorshift32(77);
  let frame = 15; // join late
  let shown = 0;
  while (!assembler.progress().complete) {
    // 60 % of the frames are readable
    if (rng.nextU32() % 10 < 6) feedFrame(assembler, encoder, frame, ALL_LANES);
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
  for (let frame = 1; frame < 4; frame += 1) feedFrame(assembler, encoder, frame, ALL_LANES);
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
  // frames 1..3 carry 63 atoms before any beacon; only the last three survive
  for (let frame = 1; frame < 4; frame += 1) feedFrame(assembler, encoder, frame, ALL_LANES);
  feedFrame(assembler, encoder, 0, ["D"]);
  assert.equal(assembler.progress().atomsReceived, 3);
  assert.equal(assembler.progress().rank, 3);
  assert.equal(new PetalStreamAssembler().limits.maxPendingAtoms, 512);
  // atoms of another stream (different tag) are buffered but never applied
  const other = new PetalStreamEncoder(payload(600, 32), 1);
  assert.notEqual(other.meta.tag, encoder.meta.tag);
  const fresh = new PetalStreamAssembler({ maxPendingAtoms: 128 });
  feedFrame(fresh, other, 1, ["F0", "F1"]);
  feedFrame(fresh, encoder, 0, ["D"]);
  assert.equal(fresh.progress().atomsReceived, 0);
});

test("corrupt atoms are caught by the payload CRC", () => {
  const data = payload(2000, 25);
  const encoder = new PetalStreamEncoder(data, 1);
  const assembler = new PetalStreamAssembler();
  feedFrame(assembler, encoder, 0, ["D", "F1"]);
  // atom 0 arrives with a valid header but a wrong body
  assembler.pushAtoms({ header: { tag: encoder.meta.tag, frame: 0 }, firstId: 0, atoms: [new Uint8Array(16).fill(0xee)] });
  let frame = 1;
  while (assembler.progress().integrityFailures === 0) {
    feedFrame(assembler, encoder, frame, ALL_LANES);
    frame += 1;
    assert.ok(frame < 100, "the corrupt atom was never caught");
  }
  assert.equal(assembler.takeCompleted(), null);
  assert.equal(assembler.progress().integrityFailures, 1);
  // clean repair frames after the reset recover the payload
  for (let next = 100; next < 600; next += 1) {
    feedFrame(assembler, encoder, next, ALL_LANES);
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
  assert.equal(d[3], 0x20);
  assert.equal(parseDLane(d.slice(0, 11)), null);
  // the retired format 1 (katakana tiles) is refused
  const bad = d.slice();
  bad[3] = 0x10;
  assert.equal(parseDLane(bad), null);
  // non-beacon frames carry an atom instead
  assert.equal(parseDLane(encoder.laneData(513).d).type, "atoms");
  assert.equal(parseAtomLane("D", encoder.laneData(513).d), null);
  assert.equal(parseAtomLane("F0", encoder.laneData(513).d), null);
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
    const lanes = choice === 0 ? ["F2"] : choice === 1 ? ["D", "F0"] : choice === 2 ? ["F1", "F3", "D"] : ALL_LANES;
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
    feedFrame(assembler, encoder, frame, ALL_LANES);
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

test("a fitted pose takes its source points onto distorted image points", () => {
  const src = PETAL_LAYOUT.finderCenters;
  const dst = [
    [380, 90],
    [900, 110],
    [880, 640],
    [360, 620],
  ];
  for (const distortion of [0, -0.08, 0.03]) {
    const pose = PetalPose.fit(src, dst, distortion, 1280, 720);
    assert.equal(pose.distortion, distortion);
    assert.deepEqual(pose.center, [640, 360]);
    assert.equal(pose.scale2, 1024 * 1024);
    src.forEach(([x, y], index) => {
      const [u, v] = pose.apply(x, y);
      assert.ok(Math.abs(u - dst[index][0]) < 1e-3 && Math.abs(v - dst[index][1]) < 1e-3, `${distortion}`);
    });
  }
  // barrel distortion pulls a mid-edge point toward the image centre
  const plain = PetalPose.fit(src, dst, 0, 1280, 720);
  const barrel = PetalPose.fit(src, dst, -0.08, 1280, 720);
  assert.ok(Math.abs(plain.apply(512, 72)[0] - barrel.apply(512, 72)[0]) < 5);
  assert.deepEqual(PetalPose.plain(PetalHomography.identity()).apply(3, 4), [3, 4]);
  assert.equal(PetalPose.fit(src.slice(0, 3), dst.slice(0, 3), 0, 1280, 720), null);
  assert.throws(() => new PetalPose([1, 0, 0, 0, 1, 0, 0, 0, 1]), TypeError);
  // without distortion the pose is the plain homography through the points
  const direct = PetalHomography.fromPoints(src, dst).values;
  plain.homography.values.forEach((value, index) => assert.ok(Math.abs(value - direct[index]) <= 1e-9 * Math.max(1, Math.abs(value))));
});

// ---------------------------------------------------------------- renderer

function renderCells(seed) {
  const d = Uint8Array.from({ length: 19 }, (_, b) => ((b * 13) & 0xff) ^ seed);
  const fields = [0, 1, 2, 3].map((sector) =>
    encodeLane(
      fieldLane(sector),
      Uint8Array.from({ length: 83 }, (_, b) => ((b * 17 + sector) & 0xff) ^ seed),
    ),
  );
  return PetalFrameCells.fromWords(encodeLane("D", d), fields);
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

test("lit petals are drawn in their colour and dark cells show the backdrop", () => {
  const frame = renderCells(2);
  const image = renderPetalFrame(frame, { size: 1024, supersample: 1 });
  const palette = PETAL_PALETTE;
  for (let cell = 0; cell < PETAL_LAYOUT.fieldCells; cell += 1) {
    const [x, y] = cellCenter(cell);
    const at = (Math.floor(y) * 1024 + Math.floor(x)) * 4;
    const pixel = [image.data[at], image.data[at + 1], image.data[at + 2]];
    let expected;
    if (frame.petals[cell] === 1) expected = cellLight(cell) ? palette.light : palette.pink;
    else expected = inMask(Math.floor(x) + 0.5, Math.floor(y) + 0.5) ? palette.backdrop : palette.background;
    assert.deepEqual(pixel, expected, `cell ${cell}`);
  }
});

test("lit dots are drawn and unlit slots are black", () => {
  const frame = renderCells(3);
  const image = renderPetalFrame(frame, { size: 1024, supersample: 1 });
  let checked = 0;
  for (let ring = 0; ring < 3; ring += 1) {
    for (let slot = 0; slot < PETAL_LAYOUT.ringSlots[ring]; slot += 1) {
      const [x, y] = slotCenter(ring, slot);
      const value = image.data[(Math.floor(y) * 1024 + Math.floor(x)) * 4];
      const flat = [0, 88, 188][ring] + slot;
      if (frame.dots[flat]) assert.ok(value > 150, `ring ${ring} slot ${slot} should be lit`);
      else assert.equal(value, 0, `ring ${ring} slot ${slot} should be dark`);
      checked += 1;
    }
  }
  assert.equal(checked, 296);
});

test("the software renderer matches the reference renderer bit for bit", () => {
  // FNV-1a 64 of the RGB bytes produced by `iroha_petal::render::render` for
  // the decoder-test stream (3,000 bytes, kind 2).
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
  const encoder = decodeSetupEncoder();
  for (const [frame, size, supersample, expected] of [
    [0, 256, 2, "b7e07f4a0f55c7e1"],
    [3, 1024, 1, "8bb66248244edead"],
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
  assert.equal(list.petals.length, cells.petals.reduce((a, b) => a + b, 0));
  assert.equal(list.dots.length, cells.dots.reduce((a, b) => a + b, 0));
  assert.equal(list.backdrop.cells.length, PETAL_LAYOUT.mask.join("").split("#").length - 1);
  assert.deepEqual([list.backdrop.x, list.backdrop.y, list.backdrop.r], [512, 512, 390]);
  assert.deepEqual(list.petal, { halfLength: 5.2, halfWidth: 4.3, notchRadius: 1.3, notchDistance: 5.4 });
  for (const finder of list.finders) {
    assert.equal(finder.core.r, 12);
    assert.equal(finder.petals.length, 5);
    assert.equal(finder.notches.length, 5);
  }
  for (const petal of list.petals) {
    assert.deepEqual([petal.x, petal.y], cellCenter(petal.index));
    assert.equal(petal.angle, cellAngle(petal.index));
    assert.equal(petal.light, cellLight(petal.index));
    assert.deepEqual(petal.fill, petal.light ? PETAL_PALETTE.light : PETAL_PALETTE.pink);
  }
  // paint the primitives in their documented order onto a grid of sample points
  const step = 3;
  const side = Math.ceil(1024 / step);
  const expected = new Array(side * side).fill(list.palette.background);
  const paint = (x0, y0, x1, y1, inside, color) => {
    for (let gy = Math.max(Math.ceil((y0 - 0.5) / step), 0); gy < side && gy * step + 0.5 <= y1; gy += 1) {
      for (let gx = Math.max(Math.ceil((x0 - 0.5) / step), 0); gx < side && gx * step + 0.5 <= x1; gx += 1) {
        if (inside(gx * step + 0.5, gy * step + 0.5)) expected[gy * side + gx] = color;
      }
    }
  };
  const inCircle = (c) => (x, y) => Math.hypot(x - c.x, y - c.y) <= c.r;
  const box = (c, r = c.r) => [c.x - r, c.y - r, c.x + r, c.y + r];
  for (const cell of list.backdrop.cells) {
    paint(cell.x, cell.y, cell.x + cell.size, cell.y + cell.size, (x, y) => x < cell.x + cell.size && y < cell.y + cell.size && inCircle(list.backdrop)(x, y), list.palette.backdrop);
  }
  const { halfLength: a, halfWidth: b, notchRadius, notchDistance } = list.petal;
  for (const petal of list.petals) {
    const cos = Math.cos(petal.angle);
    const sin = Math.sin(petal.angle);
    paint(...box(petal, 6), (x, y) => {
      const along = (x - petal.x) * cos + (y - petal.y) * sin;
      const across = -(x - petal.x) * sin + (y - petal.y) * cos;
      return (along / a) ** 2 + (across / b) ** 2 <= 1 && (along - notchDistance) ** 2 + across ** 2 > notchRadius ** 2;
    }, petal.fill);
  }
  for (const dot of list.dots) paint(...box(dot), inCircle(dot), list.palette.pink);
  for (const finder of list.finders) {
    for (const disc of [finder.core, ...finder.petals]) paint(...box(disc), inCircle(disc), list.palette.light);
    for (const notch of finder.notches) paint(...box(notch), inCircle(notch), list.palette.background);
  }
  const image = renderPetalFrame(cells, { size: 1024, supersample: 1 });
  let mismatches = 0;
  for (let gy = 0; gy < side; gy += 1) {
    for (let gx = 0; gx < side; gx += 1) {
      const at = (gy * step * 1024 + gx * step) * 4;
      const color = expected[gy * side + gx];
      if (image.data[at] !== color[0] || image.data[at + 1] !== color[1] || image.data[at + 2] !== color[2]) mismatches += 1;
    }
  }
  assert.equal(mismatches, 0);
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
  assert.deepEqual(locateCandidates(new PetalLuma(200, 200)), []);
});

test("three finders forming a corner infer the fourth", () => {
  const blob = (x, y) => ({ x, y, size: 60 });
  // top-left, top-right and bottom-left of a slightly rotated square, plus clutter
  const finders = [blob(100, 110), blob(540, 90), blob(120, 550)];
  for (let i = 0; i < 5; i += 1) finders.push({ x: 300 + 10 * i, y: 300, size: 14 });
  const triple = selectTriple(finders);
  const fourth = triple.corners[triple.inferred];
  assert.ok(Math.abs(fourth.x - 560) < 1e-9 && Math.abs(fourth.y - 530) < 1e-9, JSON.stringify(fourth));
  assert.equal(triple.inferred, 2, "the inferred corner is bottom-right in clockwise order");
  assert.equal(fourth.size, 60, "the inferred corner has the mean size of the three");
  // three blossoms in a row are no corner, and neither are fewer than three
  assert.equal(selectTriple([blob(0, 0), blob(440, 0), blob(880, 0)]), null);
  assert.equal(selectTriple([blob(100, 110), blob(540, 90)]), null);
  assert.throws(() => selectTriple(null), TypeError);
});

test("a smaller blob at the inferred corner completes the quad", () => {
  const blob = (x, y, size) => ({ x, y, size });
  // steep tilt: the far finder is under 0.55 of the largest, but it is where the fourth corner
  // belongs
  const finders = [blob(100, 100, 64), blob(540, 100, 60), blob(100, 540, 62), blob(520, 515, 30)];
  const strong = strongFinders(finders);
  assert.equal(strong.length, 3);
  const triple = selectTriple(strong);
  const full = completeTriple(finders, triple.corners, triple.inferred);
  assert.ok(full.some((f) => Math.abs(f.x - 520) < 1e-9 && Math.abs(f.y - 515) < 1e-9));
  // a blob further than 0.3 legs from the parallelogram point does not complete it
  const far = [...finders.slice(0, 3), blob(420, 420, 30)];
  assert.equal(completeTriple(far, triple.corners, triple.inferred), null);
});

/** The 512-pixel locator test render. */
function locatorFrame() {
  const encoder = new PetalStreamEncoder(new Uint8Array(200).fill(9), 1);
  return renderLuma(encoder.cells(1), 512, 2);
}

test("a hidden blossom yields an inferred candidate", () => {
  const luma = locatorFrame();
  // the candidates of a clean render start with its four seen finders
  const clean = locateCandidates(luma);
  assert.equal(clean[0].inferred, null);
  assert.deepEqual(clean[0].corners, locate(luma));
  // paint over the bottom-left blossom (centre 36, 476 at this size)
  for (let y = 420; y < 512; y += 1) luma.data.fill(0, y * 512, y * 512 + 92);
  const candidates = locateCandidates(luma);
  const inferred = candidates.find((set) => set.inferred !== null);
  assert.ok(inferred !== undefined, "an inferred candidate");
  assert.equal(candidates.indexOf(inferred), candidates.length - 1, "the inferred candidate comes last");
  const corner = inferred.corners[inferred.inferred];
  assert.ok(Math.abs(corner.x - 36) < 4 && Math.abs(corner.y - 476) < 4, JSON.stringify(corner));
});

/** A canvas-sized image with finder (lit) and reference-canvas (dark) levels painted where they are sampled. */
function levelCard(levels) {
  const data = new Uint8Array(1024 * 1024);
  const paint = (cx, cy, radius, value) => {
    for (let y = cy - radius; y <= cy + radius; y += 1) {
      for (let x = cx - radius; x <= cx + radius; x += 1) data[y * 1024 + x] = value;
    }
  };
  PETAL_LAYOUT.finderCenters.forEach(([cx, cy], corner) => {
    const [lit, dark] = levels[corner];
    const sx = cx < 512 ? 1 : -1;
    const sy = cy < 512 ? 1 : -1;
    paint(cx, cy, 30, lit);
    paint(cx + sx * 100, cy, 12, dark);
    paint(cx, cy + sy * 100, 12, dark);
  });
  return new PetalLuma(1024, 1024, data);
}

test("an inferred corner needs contrast too", () => {
  const identity = PetalPose.plain(PetalHomography.identity());
  // even light: the hidden corner (3) gets levels between the others'
  const even = referenceLevels(levelCard([[230, 30], [220, 25], [210, 20], [0, 0]]), identity, 3);
  assert.ok(even !== null && even.lit[3] - even.dark[3] >= 12);
  // the hidden corner's neighbours disagree (one dim, one veiled): the estimates cross
  const uneven = levelCard([[60, 45], [250, 20], [200, 185], [0, 0]]);
  assert.equal(referenceLevels(uneven, identity, 3), null);
  // with every corner seen, the same light is fine
  const seen = levelCard([[60, 45], [250, 20], [200, 185], [240, 20]]);
  assert.ok(referenceLevels(seen, identity) !== null);
});

test("following finds a moved blossom and refuses a lost one", () => {
  const luma = locatorFrame();
  const found = followFinder(luma, { x: 48, y: 27, size: 60 });
  assert.ok(Math.abs(found.x - 36) < 1.5 && Math.abs(found.y - 36) < 1.5, JSON.stringify(found));
  assert.equal(found.size, 60);
  // nothing bright in the canvas corner gap between a blossom and the rings
  assert.equal(followFinder(luma, { x: 100, y: 15, size: 30 }), null);
  // a blossom that moved further than 0.75 diameters is refused (here: found, but too far)
  assert.equal(followFinder(luma, { x: 36 + 70, y: 36, size: 60 }), null);
  // degenerate expectations do not crash
  assert.equal(followFinder(luma, { x: Number.NaN, y: 36, size: 60 }), null);
  assert.equal(followFinder(luma, { x: -1e9, y: 1e9, size: 60 }), null);
  for (const [x, y, size] of [
    [1e300, 36, 60],
    [-1e300, -1e300, 60],
    [36, Number.POSITIVE_INFINITY, 60],
    [36, 36, Number.NaN],
  ]) {
    assert.equal(followFinder(luma, { x, y, size }), null);
  }
  // a huge disc just covers the whole image
  followFinder(luma, { x: 36, y: 36, size: 1e300 });
});

// ---------------------------------------------------------------- frame decoder

/** The decoder tests' stream: 3,000 bytes, kind 2. */
function decodeSetupEncoder() {
  const data = Uint8Array.from({ length: 3000 }, (_, i) => (Math.imul(i, 2654435761) >>> 11) & 0xff);
  return new PetalStreamEncoder(data, 2);
}

/** A 768-pixel render (2x2 supersampled) of frame `frame` of the decoder tests' stream. */
function decodeSetup(frame) {
  const encoder = decodeSetupEncoder();
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

/** Asserts that every lane of `decoded` read the recorded data. */
function assertAllLanes(decoded, truth, name) {
  assert.deepEqual(decoded.d?.data, truth.d, `${name}: lane D`);
  for (let sector = 0; sector < 4; sector += 1) {
    assert.deepEqual(decoded.fields[sector]?.data, truth.fields[sector], `${name}: field lane ${sector}`);
  }
}

test("a clean render decodes every lane", () => {
  const { encoder, luma } = decodeSetup(5);
  const decoded = decodePetalFrame(luma);
  assertAllLanes(decoded, encoder.laneData(5), "clean");
  assert.deepEqual([decoded.rotation, decoded.mirrored, decoded.lanesOk(), decoded.letters()], [0, false, 5, "D0123"]);
  assert.ok(Object.is(decoded.pose.distortion, 0), "a clean render needs no distortion");
  assert.equal(decoded.beacon(), null);
  assert.equal(decoded.atomPackets().length, 5);
  assert.deepEqual(decoded.lane("F2"), decoded.fields[2]);
  assert.ok(observedCells(luma, decoded).equals(encoder.cells(5)));
  const at = decodePetalFrameAt(luma, decoded.pose);
  assertAllLanes(at, encoder.laneData(5), "known pose");
});

test("rotated and mirrored renders decode with the right orientation", () => {
  const { encoder, luma } = decodeSetup(7);
  const truth = encoder.laneData(7);
  // mirrored hypotheses enumerate corners in the opposite direction, so the unrotated mirror
  // reports quarter-turn index 1
  for (const [name, map, rotation, mirrored] of [
    ["rot90", (x, y, n) => [y, n - 1 - x], 1, false],
    ["rot180", (x, y, n) => [n - 1 - x, n - 1 - y], 2, false],
    ["rot270", (x, y, n) => [n - 1 - y, x], 3, false],
    ["mirror", (x, y, n) => [n - 1 - x, y], 1, true],
  ]) {
    const decoded = decodePetalFrame(transformLuma(luma, map));
    assert.equal(decoded.mirrored, mirrored, name);
    assert.equal(decoded.rotation, rotation, `${name} rotation`);
    assertAllLanes(decoded, truth, name);
  }
});

test("the field alone tells the orientation when the rings are gone", () => {
  // black out the ring band: no gates, no lane D; a wrong quarter turn reads permuted petals
  // that no field codeword accepts
  const { encoder, luma } = decodeSetup(9);
  const n = luma.width;
  const scale = n / 1024;
  for (let y = 0; y < n; y += 1) {
    for (let x = 0; x < n; x += 1) {
      const r = Math.hypot((x + 0.5) / scale - 512, (y + 0.5) / scale - 512);
      if (r >= 392 && r <= 490) luma.data[y * n + x] = 0;
    }
  }
  const rotated = transformLuma(luma, (x, y, size) => [y, size - 1 - x]);
  const decoded = decodePetalFrame(rotated);
  assert.equal(decoded.d, null);
  assert.equal(decoded.rotation, 1);
  const truth = encoder.laneData(9);
  for (let sector = 0; sector < 4; sector += 1) assert.deepEqual(decoded.fields[sector]?.data, truth.fields[sector]);
});

test("corrected counts rewritten bytes, not just erasures", () => {
  const data = Uint8Array.from({ length: 83 }, (_, b) => b);
  const word = encodeLane("F1", data);
  for (const position of [2, 11, 30, 77]) word[position] ^= 0x5a;
  const result = decodeWithErasures("F1", word, new Float64Array(word.length).fill(1));
  assert.deepEqual(result.data, data);
  assert.equal(result.erasures, 0);
  assert.equal(result.corrected, 4);
  // with the damaged bytes flagged as least confident, they become erasures
  const flagged = new Float64Array(word.length).fill(1);
  for (const position of [2, 11, 30, 77]) flagged[position] = 0;
  const again = decodeWithErasures("F1", word, flagged);
  assert.deepEqual(again.data, data);
  assert.ok(again.corrected >= 4);
});

test("random words are almost never accepted", () => {
  // Reed-Solomon with erasures can accept a word that is not a transmission. Lane D has only 11
  // parity bytes, so its schedule stops at five erasures; a field lane has 22 and stops at
  // eleven. The counts are exact so that every SDK port, fed the same xorshift32 words and
  // byte-valued confidences (many ties, so the ranking must be stable), reproduces the decoder
  // bit for bit.
  const rng = new PetalXorshift32(0x5eed);
  const trials = 40000;
  for (const [lane, expected] of [
    ["D", 3],
    ["F0", 0],
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

test("erasures stop at half the parity", () => {
  // damaged bytes: `flagged` of them marked least confident, two more hidden: beyond half the
  // parity the decoder must refuse instead of risking a wrong codeword
  for (const [lane, flagged] of [
    ["D", 7],
    ["F2", 16],
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
    repairable[24] ^= 0x3c;
    const result = decodeWithErasures(lane, repairable, halfFlagged);
    assert.deepEqual(result.data, data, `lane ${lane}`);
    assert.ok(result.erasures <= half, `lane ${lane}`);
  }
});

test("the erasure schedule has the reference's steps for every lane", () => {
  // `flagged` damaged bytes are ranked least confident, `hidden` ones look fine; a lane decodes at
  // the first step whose erasures plus twice the remaining errors fit the parity. The cases below
  // fail at every earlier step, so the step that succeeds is the one that is pinned.
  const cases = [
    // lane, flagged, hidden, erasures used (null: no step of the schedule can repair it)
    ["D", 5, 3, 5], // steps 0, 1, 2, 3, 5: the half step is the last one
    ["D", 7, 2, null], // a seventh erasure would repair it, the capped schedule must not
    ["F1", 7, 7, 7], // steps 0, 2, 5, 7, 11
    ["F1", 11, 5, 11], // the half step is the last one for a field lane too
    ["F1", 12, 5, null], // a twelfth erasure would repair it
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
  // Ties must be broken by position or ports disagree about which bytes are erased. With all
  // confidences equal the lowest positions are erased first, so damage at the lowest positions is
  // repaired as soon as the schedule reaches enough erasures; a ranking that put the ties the
  // other way round would erase good bytes instead.
  for (const [lane, damagedPositions, mask, erasures] of [
    // The reference's case: three damaged bytes at the front plus four hidden ones fit lane D only
    // if exactly the first three positions are erased (3 erasures + 4 errors = all 11 parity bytes).
    ["D", [0, 1, 2, 20, 21, 22, 23], 0x5a, 3],
    ["D", [0, 1, 2, 3, 4, 5], 0xa5, 1], // 11 parity bytes: 6 errors fail, 1 erasure + 5 errors fit
    ["F0", Array.from({ length: 13 }, (_, index) => index), 0xa5, 5], // 22: 2 + 2 x 11 fail, 5 + 2 x 8 fit
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

test("two-means splits two clusters and survives one", () => {
  const values = [...new Array(40).fill(0.05), ...new Array(60).fill(0.9)];
  const t = twoMeans(values);
  assert.ok(t > 0.3 && t < 0.6, `${t}`);
  assert.ok(Math.abs(twoMeans(new Array(10).fill(0.4)) - 0.4) < 1e-12);
  // the midpoint of the two cluster means, not the median of the values
  const skewed = [...new Array(90).fill(0), ...new Array(10).fill(1)];
  assert.ok(Math.abs(twoMeans(skewed) - 0.5) < 1e-12);
  assert.equal(twoMeans(Float64Array.of(0, 1, 9, 9), 2), 0.5, "only the first `count` values");
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
  const identity = PetalPose.plain(PetalHomography.identity());
  assert.equal(decodePetalFrameAt(new PetalLuma(8, 8), identity), null);
  // a buffer that does not match the stated size must not reach the sampler, in any entry point
  const broken = { width: 100, height: 100, data: new Uint8Array(5) };
  assert.equal(decodePetalFrameAt(broken, identity), null);
  const { luma } = decodeSetup(5);
  const decoded = decodePetalFrame(luma);
  assert.equal(observedCells(broken, decoded), null);
  assert.equal(fieldEvidence(broken, decoded), null);
  // ... and so must a frame that is too small or over the pixel budget
  for (const [image, options] of [
    [new PetalLuma(47, 400), undefined],
    [luma, { maxPixels: 1000 }],
  ]) {
    assert.equal(observedCells(image, decoded, options), null);
    assert.equal(fieldEvidence(image, decoded, options), null);
    assert.equal(decodePetalFrameAt(image, decoded.pose, options), null);
  }
  assert.throws(() => decodePetalFrame({ width: 64, height: 64, data: [] }), TypeError);
  assert.throws(() => decodePetalFrame(new PetalLuma(64, 64), { tryMirrored: "yes" }), TypeError);
  assert.throws(() => decodePetalFrameAt(luma, PetalHomography.identity()), TypeError);
  assert.throws(() => fieldEvidence(luma, { pose: null }), TypeError);
});

test("observed cells match the drawn cells of a clean render", () => {
  const { encoder, luma } = decodeSetup(3);
  const decoded = decodePetalFrame(luma);
  assert.ok(observedCells(luma, decoded).equals(encoder.cells(3)));
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
  // NaN and infinite poses behave as in the reference: no crash, and the non-finite levels they
  // produce are refused
  const noisy = new PetalLuma(320, 240, payload(320 * 240, 5));
  const lanes = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1e300, 0, 0, 0, 1e300, 0, 0, 0, 1e-300],
    [Number.NaN, 0, 0, 0, 1, 0, 0, 0, 1],
  ].map((m) => decodePetalFrameAt(noisy, PetalPose.plain(new PetalHomography(m)))?.lanesOk() ?? null);
  assert.deepEqual(lanes, [null, null, null]);
});

test("random blob scenes never crash or yield lanes", () => {
  // Scenes with several random bright ellipses (some finder-sized) on noise: exercises the
  // locator, quad and corner selection, the inferred-corner search and the pose fit on
  // degenerate layouts.
  const rng = new PetalXorshift32(2024);
  for (let scene = 0; scene < 60; scene += 1) {
    const w = 160 + (rng.nextU32() % 400);
    const h = 120 + (rng.nextU32() % 300);
    const data = new Uint8Array(w * h);
    for (let index = 0; index < w * h; index += 1) data[index] = rng.nextU32() % 40;
    const blobs = 3 + (rng.nextU32() % 8);
    for (let blob = 0; blob < blobs; blob += 1) {
      const cx = rng.nextU32() % w;
      const cy = rng.nextU32() % h;
      const rx = 6 + (rng.nextU32() % 40);
      const ry = 6 + (rng.nextU32() % 40);
      for (let y = 0; y < h; y += 1) {
        for (let x = 0; x < w; x += 1) {
          const dx = (x - cx) / rx;
          const dy = (y - cy) / ry;
          if (dx * dx + dy * dy <= 1) data[y * w + x] = 230;
        }
      }
    }
    // a lucky layout may locate finders but cannot yield lanes
    try {
      assert.equal(decodePetalFrame(new PetalLuma(w, h, data)).lanesOk(), 0, `scene ${scene} produced lane data from blobs`);
    } catch (error) {
      assert.ok(error instanceof PetalError, `scene ${scene}: ${error}`);
    }
  }
});

test("a large hidden region never reads wrong data", () => {
  // the whole bottom-right quarter is gone: rings and petals with it
  const { encoder, luma } = decodeSetup(2);
  const n = luma.width;
  for (let y = (n * 3) / 4; y < n; y += 1) luma.data.fill(0, y * n + (n * 3) / 4, (y + 1) * n);
  const truth = encoder.laneData(2);
  let decoded = null;
  try {
    decoded = decodePetalFrame(luma);
  } catch (error) {
    assert.ok(error instanceof PetalError);
  }
  if (decoded !== null) {
    for (const lane of ALL_LANES) {
      const result = decoded.lane(lane);
      if (result !== null) assert.deepEqual(result.data, laneBytes(truth, lane), `lane ${lane}`);
    }
  }
});

/** A 768-pixel render of frame `frameNo` with the blossom of canonical corner `corner` painted over. */
function hiddenBlossom(frameNo, corner) {
  const { encoder, luma } = decodeSetup(frameNo);
  const n = luma.width;
  const scale = n / PETAL_LAYOUT.canvas;
  const [fx, fy] = PETAL_LAYOUT.finderCenters[corner];
  const cx = fx * scale;
  const cy = fy * scale;
  const radius = 75 * scale;
  for (let y = 0; y < n; y += 1) {
    for (let x = 0; x < n; x += 1) {
      const dx = x + 0.5 - cx;
      const dy = y + 0.5 - cy;
      if (dx * dx + dy * dy <= radius * radius) luma.data[y * n + x] = 0;
    }
  }
  return { encoder, luma };
}

test("a hidden blossom is inferred and every lane still reads", () => {
  for (let corner = 0; corner < 4; corner += 1) {
    const { encoder, luma } = hiddenBlossom(2, corner);
    const decoded = decodePetalFrame(luma);
    assert.equal(decoded.inferredCorner, corner, `corner ${corner}`);
    assert.deepEqual([decoded.rotation, decoded.mirrored], [0, false], `corner ${corner}`);
    assertAllLanes(decoded, encoder.laneData(2), `corner ${corner}`);
    // the diagnostics extrapolate the hidden corner's levels like the decoder; without the
    // inferred corner the missing blossom has no contrast and there are no levels at all
    assert.ok(observedCells(luma, decoded) !== null);
    assert.equal(observedCells(luma, { pose: decoded.pose }), null);
    assert.equal(fieldEvidence(luma, { pose: decoded.pose, inferredCorner: 4 }), null);
  }
});

test("the inferred corner is reported in code coordinates when mirrored", () => {
  // hide the top-right blossom of the code, then mirror the picture: the hidden blossom appears
  // top-left in the image but is still corner 1 of the code
  const { encoder, luma } = hiddenBlossom(3, 1);
  const decoded = decodePetalFrame(transformLuma(luma, (x, y, n) => [n - 1 - x, y]));
  assert.ok(decoded.mirrored);
  assert.equal(decoded.inferredCorner, 1);
  assert.deepEqual(decoded.d.data, encoder.laneData(3).d);
});

/**
 * The image warped by barrel distortion about the image centre, as a cheap lens does: pixel `p`
 * shows what an undistorted camera saw at `c + (p - c) / (1 + k r^2 / s^2)`.
 */
function barrel(image, k) {
  const { width: w, height: h } = image;
  const cx = w / 2;
  const cy = h / 2;
  const scale2 = (0.8 * Math.max(w, h)) ** 2;
  const out = new PetalLuma(w, h);
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const dx = x + 0.5 - cx;
      const dy = y + 0.5 - cy;
      // invert p_d = c + u (1 + k |u|^2 / s^2) for the undistorted offset u
      let ux = dx;
      let uy = dy;
      for (let round = 0; round < 8; round += 1) {
        const f = 1 + (k * (ux * ux + uy * uy)) / scale2;
        ux = dx / f;
        uy = dy / f;
      }
      out.data[y * w + x] = Math.round(image.sample(cx + ux, cy + uy));
    }
  }
  return out;
}

test("lens distortion is found when the plain pose misses the petals", () => {
  // a small, strongly distorted picture: 384 px of code with the corner blossoms far out
  const { encoder, luma } = decodeSetup(6);
  // downscale 768 -> 384 by averaging 2x2 blocks, then frame it in 640x480
  const small = new PetalLuma(640, 480);
  for (let y = 0; y < 384; y += 1) {
    for (let x = 0; x < 384; x += 1) {
      const at = 2 * y * 768 + 2 * x;
      const sum = luma.data[at] + luma.data[at + 1] + luma.data[at + 768] + luma.data[at + 769];
      small.data[(y + 48) * 640 + x + 128] = Math.floor((sum + 2) / 4);
    }
  }
  const distorted = barrel(small, -0.12);
  // the plain pose misses petals at the edge of the field
  const corners = locate(distorted).map((finder) => [finder.x, finder.y]);
  const plain = decodePetalFrameAt(distorted, PetalPose.fit(PETAL_LAYOUT.finderCenters, corners, 0, 640, 480));
  assert.ok(plain.fields.some((field) => field === null), plain.letters());
  const decoded = decodePetalFrame(distorted);
  assert.ok(decoded.pose.distortion < 0, `${decoded.pose.distortion}`);
  assertAllLanes(decoded, encoder.laneData(6), "barrel");
});

/** Shifts a luma image by whole pixels, filling with black. */
function shifted(image, dx, dy) {
  const { width, height } = image;
  const out = new PetalLuma(width, height);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const sx = x - dx;
      const sy = y - dy;
      if (sx >= 0 && sy >= 0 && sx < width && sy < height) out.data[y * width + x] = image.data[sy * width + sx];
    }
  }
  return out;
}

/** Places a luma image in the middle of a larger black frame. */
function padded(image, pad) {
  const width = image.width + 2 * pad;
  const out = new PetalLuma(width, image.height + 2 * pad);
  for (let y = 0; y < image.height; y += 1) {
    out.data.set(image.data.subarray(y * image.width, (y + 1) * image.width), (y + pad) * width + pad);
  }
  return out;
}

test("tracking follows a small movement and gives up on a jump", () => {
  const { encoder, luma: render } = decodeSetup(6);
  const luma = padded(render, 100);
  const first = decodePetalFrame(luma);
  const followed = trackPetalFrame(shifted(luma, 9, -6), first);
  assert.ok(followed !== null, "tracks a 9 px move");
  assertAllLanes(followed, encoder.laneData(6), "tracked");
  assert.equal(followed.inferredCorner, null);
  assert.deepEqual([followed.rotation, followed.mirrored], [first.rotation, first.mirrored]);
  // more than a finder diameter: tracking refuses, a full decode is needed
  const jumped = shifted(luma, 95, 0);
  assert.equal(trackPetalFrame(jumped, first), null);
  assert.ok(decodePetalFrame(jumped).lanesOk() > 0);
  // unusable images and foreign previous frames
  assert.equal(trackPetalFrame(new PetalLuma(47, 400), first), null);
  assert.equal(trackPetalFrame(luma, first, { maxPixels: 1000 }), null);
  assert.throws(() => trackPetalFrame(luma, { pose: first.pose }), TypeError);
  assert.throws(() => trackPetalFrame({ width: 64, height: 64, data: [] }, first), TypeError);
});

test("broken poses are refused without crashing", () => {
  const { luma } = decodeSetup(4);
  const previous = decodePetalFrame(luma);
  assert.ok(trackPetalFrame(luma, previous) !== null);
  const rebuilt = (pose, inferred) =>
    new PetalDecodedFrame(pose, previous.rotation, previous.mirrored, previous.d, previous.fields, inferred);
  // a good pose that names a corner that does not exist
  for (const corner of [4, 255]) {
    assert.equal(trackPetalFrame(luma, rebuilt(previous.pose, corner)), null);
  }
  const nonFinite = [new Array(9).fill(Number.NaN), [Number.POSITIVE_INFINITY, 0, 0, 0, 1, 0, 0, 0, 1]];
  for (const broken of nonFinite) {
    assert.equal(decodePetalFrameAt(luma, PetalPose.plain(new PetalHomography(broken))), null);
  }
  // the last one makes every finder far larger than the image
  const huge = [50, 0, 0, 0, 50, 0, 0, 0, 1];
  for (const broken of [...nonFinite, huge]) {
    for (const inferred of [null, 2]) {
      const frame = rebuilt(PetalPose.plain(new PetalHomography(broken)), inferred);
      assert.equal(trackPetalFrame(luma, frame), null, `${broken.join(",")} inferred ${inferred}`);
    }
  }
});

test("tracking survives a blossom that disappears", () => {
  const { luma } = decodeSetup(4);
  const first = decodePetalFrame(luma);
  // the same code, slightly moved, now with the bottom-left blossom covered
  const { encoder, luma: covered } = hiddenBlossom(4, 3);
  const followed = trackPetalFrame(shifted(covered, -5, 4), first);
  assert.ok(followed !== null, "tracks with three blossoms");
  assert.equal(followed.inferredCorner, 3);
  assert.deepEqual(followed.d.data, encoder.laneData(4).d);
});

test("a blossom that reappears is seen again", () => {
  const { luma: covered } = hiddenBlossom(4, 3);
  const first = decodePetalFrame(covered);
  assert.equal(first.inferredCorner, 3);
  // the thumb moves away and the hand moves a little
  const { encoder, luma } = decodeSetup(4);
  const followed = trackPetalFrame(shifted(luma, 4, -3), first);
  assert.ok(followed !== null, "tracks");
  assert.equal(followed.inferredCorner, null);
  assert.deepEqual(followed.d.data, encoder.laneData(4).d);
  // still covered: still inferred
  const still = trackPetalFrame(shifted(covered, 4, -3), first);
  assert.ok(still !== null, "tracks");
  assert.equal(still.inferredCorner, 3);
});

// ---------------------------------------------------------------- field evidence and the local read

test("merging one capture reads the field lanes decode read", () => {
  for (const [name, { luma }] of [
    ["clean", decodeSetup(5)],
    ["hidden corner", hiddenBlossom(2, 3)],
  ]) {
    const frame = decodePetalFrame(luma);
    assert.ok(frame.fields.every((field) => field !== null), name);
    const evidence = fieldEvidence(luma, frame);
    assert.equal(evidence.levels.length, PETAL_LAYOUT.fieldCells);
    assert.deepEqual(mergeFields([evidence]), frame.fields, name);
  }
});

test("averaging captures cancels their independent noise", () => {
  const { encoder, luma } = decodeSetup(5);
  const truth = encoder.laneData(5);
  const frame = decodePetalFrame(luma);
  const clean = fieldEvidence(luma, frame);
  const rng = new PetalXorshift32(9);
  // uniform noise of +-0.6 of the lit level per cell: about one bit in eight flips in one capture,
  // far more than a field codeword corrects
  const captures = Array.from(
    { length: 4 },
    () => new PetalFieldEvidence(clean.levels.map((value) => value + (rng.nextByte() - 127.5) / 213)),
  );
  for (const capture of captures) {
    assert.ok(
      mergeFields([capture]).every((field) => field === null),
      "one noisy capture must not read a field lane, or the test proves nothing",
    );
  }
  const merged = mergeFields(captures);
  merged.forEach((result, sector) => assert.deepEqual(result?.data, truth.fields[sector], `sector ${sector}`));
  // skipped sectors are not read
  const skipped = mergeFields(captures, [true, false, true, true]);
  assert.ok(skipped[0] === null && skipped[1] !== null && skipped[2] === null);
});

test("evidence needs a usable image and a real corner", () => {
  const { luma } = decodeSetup(5);
  const frame = decodePetalFrame(luma);
  assert.equal(fieldEvidence(new PetalLuma(20, 20), frame), null);
  const invalid = new PetalDecodedFrame(frame.pose, frame.rotation, frame.mirrored, frame.d, frame.fields, 7);
  assert.equal(fieldEvidence(luma, invalid), null);
  assert.deepEqual(mergeFields([]), [null, null, null, null]);
  assert.throws(() => mergeFields([{ levels: new Float64Array(3376) }]), TypeError);
  assert.throws(() => new PetalFieldEvidence(new Float64Array(10)), TypeError);
});

test("neighbourhoods hold every cell within 32 units, itself included", () => {
  const { start, cells } = neighbourhoods();
  assert.equal(start.length, PETAL_LAYOUT.fieldCells + 1);
  for (const cell of [0, 1, 100, 1500, 3375]) {
    const [x, y] = cellCenter(cell);
    const around = Array.from(cells.subarray(start[cell], start[cell + 1]));
    const brute = [];
    for (let other = 0; other < PETAL_LAYOUT.fieldCells; other += 1) {
      const [ox, oy] = cellCenter(other);
      if ((ox - x) ** 2 + (oy - y) ** 2 <= 1024) brute.push(other);
    }
    assert.deepEqual(around, brute, `cell ${cell}`);
    assert.ok(around.includes(cell));
  }
});

test("the local read cancels light the finder levels cannot describe", () => {
  const { encoder, luma } = decodeSetup(5);
  const truth = encoder.laneData(5);
  const levels = fieldEvidence(luma, decodePetalFrame(luma)).levels;
  // a soft shadow band down the right of the code and veiling light that grows towards the
  // bottom: every cell's level gets the gain and offset of where it sits
  const lit = levels.map((value, cell) => {
    const [x, y] = cellCenter(cell);
    return (1 - 0.65 * Math.exp(-(((x - 700) / 80) ** 2))) * value + 0.3 * (y / 1024);
  });
  const level = [null, null, null, null];
  readSectors(lit, NO_SKIP, level);
  // the finder levels lose the lanes under the shadow and keep the others
  assert.deepEqual(level.map((field) => field !== null), [false, true, true, false]);
  const local = [null, null, null, null];
  readSectors(localLevels(lit), NO_SKIP, local);
  local.forEach((result, sector) => assert.deepEqual(result?.data, truth.fields[sector], `local read, sector ${sector}`));
  // the combined read keeps the level read's lanes and fills in the rest from the local levels
  const combined = [null, null, null, null];
  readFields(lit, NO_SKIP, combined);
  combined.forEach((result, sector) => {
    assert.deepEqual(result?.data, truth.fields[sector], `combined read, sector ${sector}`);
    if (level[sector] !== null) assert.deepEqual(result, level[sector], `sector ${sector} keeps the level read`);
  });
  // a local level of 0 and 1 sits at the means of a neighbourhood's darkest and brightest cells
  const flat = localLevels(new Float64Array(PETAL_LAYOUT.fieldCells).fill(0.5));
  assert.ok(flat.every((value) => value === 0), "a flat field has no contrast: a 0.05 span, centred at 0");
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
  const data = payload(5000, 3);
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
  assert.ok(stats.readable > 0 && stats.laneD > 0 && stats.fieldLanes > 0);
  assert.ok(stats.frames >= stats.located && stats.located >= stats.readable);
  // 5,000 bytes need 313 atoms: a clean camera reads them in 15 or 16 frames
  assert.ok(stats.frames <= 17, JSON.stringify(stats));
});

test("a steady camera is tracked after the first frame", () => {
  const encoder = new PetalStreamEncoder(payload(3000, 3), 2);
  const shoot = (frame) =>
    cameraCapture(renderLuma(encoder.cells(frame), 512, 2), { width: 640, height: 480, rotationDeg: 8, seed: frame + 1 });
  const session = new PetalScanSession();
  for (let frame = 0; frame < 6; frame += 1) {
    const outcome = session.push(shoot(frame), frame * 125);
    assert.equal(outcome.error, null, `frame ${frame}`);
  }
  let stats = session.stats();
  assert.equal(stats.readable, 6);
  assert.equal(stats.tracked, 5, "every frame after the first follows the pose");
  assert.equal(stats.inferred, 0);
  // a pause longer than the tracking window forces a full search again
  session.push(shoot(6), 5 * 125 + PETAL_TRACK_WINDOW_MS + 1);
  stats = session.stats();
  assert.deepEqual([stats.tracked, stats.readable], [5, 7]);
  // so does a reset, which forgets the pose
  session.reset();
  session.push(shoot(7), 5 * 125 + PETAL_TRACK_WINDOW_MS + 100);
  stats = session.stats();
  assert.deepEqual([stats.tracked, stats.readable], [5, 8]);
  assert.equal(PETAL_TRACK_WINDOW_MS, 500);
});

test("a session counts frames read with a hidden corner and keeps tracking through one", () => {
  const session = new PetalScanSession();
  const { luma } = decodeSetup(4);
  assert.equal(session.push(luma, 0).lanes, "D0123");
  // a thumb arrives over the bottom-left blossom: tracked, with the corner inferred
  const outcome = session.push(hiddenBlossom(4, 3).luma, 100);
  assert.equal(outcome.lanes, "D0123");
  assert.deepEqual([session.stats().tracked, session.stats().inferred], [1, 1]);
  assert.deepEqual([session.stats().laneD, session.stats().fieldLanes], [2, 8]);
  // a frame without a code keeps the pose, and an unreadable frame is not tracked
  assert.equal(session.push(new PetalLuma(768, 768), 200).error, "no_finders");
  assert.deepEqual([session.stats().tracked, session.stats().readable], [1, 2]);
});

test("idle sessions forget partial streams", () => {
  const encoder = new PetalStreamEncoder(payload(12000, 3), 1);
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
  assert.equal(outcome.merged, "");
  assert.equal(session.stats().frames, 1);
  assert.equal(session.stats().located, 0);
  assert.equal(PETAL_DEFAULT_SCAN_LIMITS.idleTimeoutMs, 30000);
  assert.equal(PETAL_DEFAULT_SCAN_LIMITS.absoluteTimeoutMs, 180000);
  assert.equal(PETAL_DEFAULT_SCAN_LIMITS.assembler.maxPendingAtoms, 512);
  assert.deepEqual(PETAL_DEFAULT_DECODE_OPTIONS, { tryMirrored: true, maxPixels: 12000000 });
});

// ---------------------------------------------------------------- shared fixtures

const streamFixture = JSON.parse(readFileSync(STREAM_FIXTURE, "utf8"));

test("fixture: constants and layout match", () => {
  const doc = streamFixture;
  assert.equal(doc.fixture_version, 2);
  assert.equal(doc.format, "petal-stream-v2");
  const c = doc.constants;
  assert.deepEqual(
    [
      c.canvas,
      c.atom_len,
      c.d_word,
      c.d_parity,
      c.field_word,
      c.field_parity,
      c.field_atoms,
      c.sectors,
      c.atoms_per_frame,
      c.field_cells,
      c.beacon_interval,
      c.format_version,
    ],
    [
      PETAL_LAYOUT.canvas,
      PETAL_STREAM.atomLen,
      PETAL_LANES.D.wordLen,
      PETAL_LANES.D.parityLen,
      PETAL_LANES.F0.wordLen,
      PETAL_LANES.F0.parityLen,
      PETAL_LANES.F0.atoms,
      PETAL_LAYOUT.sectors,
      PETAL_STREAM.atomsPerFrame,
      PETAL_LAYOUT.fieldCells,
      PETAL_STREAM.beaconInterval,
      PETAL_STREAM.formatVersion,
    ],
  );
  const layout = doc.layout;
  assert.deepEqual(layout.finder_centers, PETAL_LAYOUT.finderCenters.flat());
  assert.deepEqual(
    [
      layout.band_inner,
      layout.band_spacing,
      layout.petal_radius,
      layout.petal_half_length,
      layout.petal_half_width,
      layout.petal_notch_radius,
      layout.petal_notch_distance,
      layout.mask_origin,
      layout.mask_pitch,
      layout.backdrop_radius,
      layout.dot_radius,
    ],
    [
      PETAL_LAYOUT.bandInner,
      PETAL_LAYOUT.bandSpacing,
      PETAL_LAYOUT.petalRadius,
      PETAL_LAYOUT.petalHalfLength,
      PETAL_LAYOUT.petalHalfWidth,
      PETAL_LAYOUT.petalNotchRadius,
      PETAL_LAYOUT.petalNotchDistance,
      PETAL_LAYOUT.maskOrigin,
      PETAL_LAYOUT.maskPitch,
      PETAL_LAYOUT.backdropRadius,
      PETAL_LAYOUT.dotRadius,
    ],
  );
  assert.deepEqual(layout.band_slots, PETAL_LAYOUT.bandSlots);
  assert.equal(layout.sector_cells.length, 4);
  layout.sector_cells.forEach((cells, sector) => assert.deepEqual(cells, sectorCells(sector), `sector ${sector}`));
  assert.deepEqual(layout.mask, PETAL_LAYOUT.mask);
  assert.deepEqual(
    cellFlags(fromHex(layout.light_cells_hex)),
    Array.from({ length: PETAL_LAYOUT.fieldCells }, (_, cell) => (cellLight(cell) ? 1 : 0)),
  );
  assert.deepEqual(layout.ring_radii, PETAL_LAYOUT.ringRadii);
  assert.deepEqual(layout.ring_slots, PETAL_LAYOUT.ringSlots);
  assert.deepEqual(layout.data_slots, dataSlots());
  const roles = slotRoles();
  const of = (kind) => roles.map((role, index) => [role.kind, index]).filter(([k]) => k === kind).map(([, index]) => index);
  assert.deepEqual(layout.gate_slots, of("gate"));
  assert.deepEqual(layout.guard_slots, of("guard"));
  assert.deepEqual(doc.palette, PETAL_PALETTE);
});

test("fixture: checksums, PRNG, mixer and whitening match", () => {
  for (const entry of streamFixture.crc32c) assert.equal(crc32c(fromHex(entry.input_hex)), entry.crc32c);
  const rng = new PetalXorshift32(1);
  assert.deepEqual(
    Array.from({ length: 6 }, () => rng.nextU32()),
    streamFixture.prng.xorshift32_seed1,
  );
  for (const entry of streamFixture.prng.mix32) assert.equal(mix32(entry.in), entry.out);
  assert.deepEqual(Object.keys(streamFixture.whitening), ALL_LANES);
  for (const lane of ALL_LANES) assert.equal(hex(laneWhitening(lane)), streamFixture.whitening[lane], lane);
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
  assert.deepEqual(streamFixture.streams.map((stream) => stream.name), ["tiny", "one-pass", "wrap"]);
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
      const label = `${stream.name} frame ${frame.frame}`;
      const lanes = encoder.laneData(frame.frame);
      assert.equal(hex(lanes.d), frame.d_data, label);
      assert.deepEqual(lanes.fields.map(hex), frame.field_data, label);
      assert.equal(hex(encodeLane("D", lanes.d)), frame.d_word, label);
      lanes.fields.forEach((field, sector) => assert.equal(hex(encodeLane(fieldLane(sector), field)), frame.field_words[sector], `${label} sector ${sector}`));
      const words = encoder.words(frame.frame);
      assert.deepEqual([hex(words.d), words.fields.map(hex)], [frame.d_word, frame.field_words], label);
      const cells = PetalFrameCells.fromWords(fromHex(frame.d_word), frame.field_words.map(fromHex));
      assert.deepEqual(
        Array.from(cells.dots).flatMap((lit, index) => (lit ? [index] : [])),
        frame.lit_dots,
        label,
      );
      assert.deepEqual(Array.from(cells.petals), cellFlags(fromHex(frame.lit_petals_hex)), label);
      assert.ok(cells.equals(encoder.cells(frame.frame)), label);
    }
    // push every fixture frame through decodeLane + assembler
    if (stream.name === "one-pass") {
      const assembler = new PetalStreamAssembler();
      for (const frame of stream.frames) {
        assembler.pushDLane(parseDLane(decodeLane("D", fromHex(frame.d_word))));
        frame.field_words.forEach((word, sector) => {
          const lane = fieldLane(sector);
          assembler.pushAtoms(parseAtomLane(lane, decodeLane(lane, fromHex(word))));
        });
      }
      assert.deepEqual(assembler.takeCompleted().payload, data);
    }
  }
});

// ---------------------------------------------------------------- golden captures

const captureFixture = JSON.parse(readFileSync(CAPTURE_FIXTURE, "utf8"));

function captureLuma(entry, key = "luma_zlib_base64") {
  const data = new Uint8Array(inflateSync(Buffer.from(entry[key], "base64")));
  return new PetalLuma(entry.width, entry.height, data);
}

/** Checks every decoded lane against the recorded data and returns the lane letters. */
function readLanes(name, frame, entry) {
  for (const lane of ALL_LANES) {
    const result = frame.lane(lane);
    const expected = lane === "D" ? entry.d_data : entry.field_data[Number(lane[1])];
    if (result !== null) assert.equal(hex(result.data), expected, `${name}: lane ${laneLetter(lane)} data`);
  }
  return frame.letters();
}

test("golden captures decode as recorded", (t) => {
  assert.equal(captureFixture.fixture_version, 2);
  const assembler = new PetalStreamAssembler();
  for (const capture of captureFixture.captures) {
    const image = captureLuma(capture);
    const started = performance.now();
    const decoded = decodePetalFrame(image);
    const elapsed = performance.now() - started;
    assert.equal(decoded.mirrored, capture.mirrored, `${capture.name}: mirror flag`);
    const lanes = readLanes(capture.name, decoded, capture);
    for (const letter of capture.must_decode) {
      assert.ok(lanes.includes(letter), `${capture.name}: required lane ${letter} was not decoded`);
    }
    // the decoder follows the reference step by step, so under V8 it reads exactly the lanes
    // the reference read
    assert.equal(lanes, capture.reference_decoded, `${capture.name}: lanes differ from the reference`);
    assert.equal(decoded.inferredCorner, capture.inferred_corner, `${capture.name}: inferred corner`);
    t.diagnostic(
      `${capture.name} ${capture.width}x${capture.height}: lanes ${lanes}, distortion ${decoded.pose.distortion} in ${elapsed.toFixed(1)} ms`,
    );
    decoded.feed(assembler);
  }
  // captures of different frames of the same stream accumulate in one assembler
  assert.ok(assembler.progress().atomsReceived > 100);
});

test("golden tracks follow the pose into the next frame", (t) => {
  assert.ok(captureFixture.tracks.length >= 2, "the fixture holds the tracking pairs");
  for (const pair of captureFixture.tracks) {
    const previous = decodePetalFrame(captureLuma(pair, "from_luma_zlib_base64"));
    const next = captureLuma(pair, "to_luma_zlib_base64");
    const started = performance.now();
    const followed = trackPetalFrame(next, previous);
    const elapsed = performance.now() - started;
    assert.ok(followed !== null, `${pair.name}: tracking lost the code`);
    const lanes = readLanes(pair.name, followed, pair);
    for (const letter of pair.must_track) assert.ok(lanes.includes(letter), `${pair.name}: lane ${letter} lost`);
    assert.equal(lanes, pair.reference_tracked, `${pair.name}: lanes differ from the reference`);
    assert.equal(followed.inferredCorner, pair.inferred_corner, `${pair.name}: inferred corner`);
    assert.deepEqual([followed.rotation, followed.mirrored], [previous.rotation, previous.mirrored]);
    t.diagnostic(`${pair.name}: tracked lanes ${lanes} in ${elapsed.toFixed(1)} ms`);
    // a scan session that saw the first frame reads the second by tracking
    const session = new PetalScanSession();
    session.push(captureLuma(pair, "from_luma_zlib_base64"), 0);
    const outcome = session.push(next, 125);
    assert.equal(outcome.lanes, pair.reference_tracked, pair.name);
    assert.equal(session.stats().tracked, 1, pair.name);
    assert.equal(session.stats().inferred, pair.inferred_corner === null ? 0 : 1, pair.name);
  }
});

test("the fixture holds the inferred-corner captures", () => {
  const inferred = captureFixture.captures.filter((capture) => capture.inferred_corner !== null);
  assert.deepEqual(
    inferred.map((capture) => [capture.name, capture.inferred_corner]),
    [
      ["hidden-corner-540p", 3],
      ["cut-corner-720p", 2],
    ],
  );
  for (const capture of captureFixture.captures) assert.ok("inferred_corner" in capture, capture.name);
});

function namedCapture(name) {
  const capture = captureFixture.captures.find((entry) => entry.name === name);
  assert.ok(capture !== undefined, `capture ${name} is part of the fixture`);
  return capture;
}

test("the barrel capture needs the distortion search", () => {
  const capture = namedCapture("barrel-480p");
  const image = captureLuma(capture);
  const decoded = decodePetalFrame(image);
  assert.ok(decoded.pose.distortion < 0, `${decoded.pose.distortion}`);
  // the same finders under a plain pose leave field lanes unread
  const corners = locate(image).map((finder) => [finder.x, finder.y]);
  const quad = [0, 1, 2, 3].map((index) => corners[(index + decoded.rotation) % 4]);
  const plain = [null, null, null, null];
  readSectors(
    fieldEvidence(image, { pose: PetalPose.fit(PETAL_LAYOUT.finderCenters, quad, 0, image.width, image.height) }).levels,
    NO_SKIP,
    plain,
  );
  assert.ok(plain.some((field) => field === null), "the plain pose must miss a lane, or the test proves nothing");
});

test("the shadow band capture needs the local read", () => {
  const capture = namedCapture("shadow-band-540p");
  const image = captureLuma(capture);
  const decoded = decodePetalFrame(image);
  const levels = fieldEvidence(image, decoded).levels;
  const level = [null, null, null, null];
  readSectors(levels, NO_SKIP, level);
  const required = [...capture.must_decode].filter((letter) => letter !== "D").map(Number);
  assert.ok(required.some((sector) => level[sector] === null), "the finder levels alone must miss a required lane");
  const combined = [null, null, null, null];
  readFields(levels, NO_SKIP, combined);
  for (const sector of required) assert.equal(hex(combined[sector].data), capture.field_data[sector], `sector ${sector}`);
  assert.deepEqual(combined, decoded.fields);
});

test("a scan session reads the recorded lanes of every golden capture", () => {
  for (const capture of captureFixture.captures) {
    const session = new PetalScanSession();
    const outcome = session.push(captureLuma(capture), 0);
    assert.equal(outcome.error, null, capture.name);
    assert.equal(outcome.lanes, capture.reference_decoded, capture.name);
    const stats = session.stats();
    const letters = capture.reference_decoded;
    assert.deepEqual([stats.laneD, stats.fieldLanes], [letters.includes("D") ? 1 : 0, letters.replace("D", "").length], capture.name);
    assert.deepEqual([stats.tracked, stats.inferred], [0, capture.inferred_corner === null ? 0 : 1], capture.name);
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
    for (const lane of ALL_LANES) {
      const data = fromHex(lane === "D" ? capture.d_data : capture.field_data[Number(lane[1])]);
      assert.deepEqual(decodeLane(lane, encodeLane(lane, data)), data, `${capture.name} lane ${lane}`);
    }
    const expected = encoder.laneData(capture.frame);
    assert.deepEqual([hex(expected.d), expected.fields.map(hex)], [capture.d_data, capture.field_data], capture.name);
  }
});

// ---------------------------------------------------------------- browser glue

function recordingContext(canvas = { width: 512, height: 512 }) {
  const calls = [];
  const context = { canvas, calls };
  for (const name of ["save", "restore", "translate", "scale", "fillRect", "beginPath", "moveTo", "lineTo", "arc", "ellipse", "closePath", "rect", "clip", "fill", "stroke"]) {
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

test("drawPetalFrame paints the draw list: backdrop, notched petals, dots, finders", () => {
  const encoder = new PetalStreamEncoder(payload(300, 8), 1);
  const cells = encoder.cells(3);
  const list = petalDrawList(cells);
  const context = recordingContext();
  drawPetalFrame(context, list);
  const count = (name) => context.calls.filter((call) => call[0] === name).length;
  assert.deepEqual(context.calls.find((call) => call[0] === "scale"), ["scale", 0.5, 0.5]);
  const petals = list.petals.length;
  // finder cores, petals and notches, the backdrop disc, the dots and one notch arc per petal
  assert.equal(count("arc"), 4 * 11 + 1 + list.dots.length + petals);
  assert.equal(count("ellipse"), petals);
  assert.equal(count("closePath"), petals);
  assert.equal(count("rect"), list.backdrop.cells.length);
  assert.equal(count("clip"), 1);
  assert.equal(count("fill"), 6);
  assert.equal(count("stroke"), 0);
  assert.equal(count("save"), count("restore"));
  assert.equal(count("moveTo"), 4 * 11 + 1 + list.dots.length + petals);
  // every petal path starts where its ellipse arc starts, so no stray edge joins two petals
  const first = context.calls.findIndex((call) => call[0] === "ellipse");
  const [, x, y, a, b, angle, start] = context.calls[first];
  const [, mx, my] = context.calls[first - 1];
  const along = a * Math.cos(start);
  const across = b * Math.sin(start);
  assert.ok(Math.abs(mx - (x + along * Math.cos(angle) - across * Math.sin(angle))) < 1e-9);
  assert.ok(Math.abs(my - (y + along * Math.sin(angle) + across * Math.cos(angle))) < 1e-9);
  // the notch arc meets the ellipse where the arc starts and ends
  const [, nx, ny, r, from, to, anticlockwise] = context.calls[first + 1];
  assert.equal(anticlockwise, true);
  const end = context.calls[first][7];
  const ex = x + a * Math.cos(end) * Math.cos(angle) - b * Math.sin(end) * Math.sin(angle);
  const ey = y + a * Math.cos(end) * Math.sin(angle) + b * Math.sin(end) * Math.cos(angle);
  assert.ok(Math.hypot(nx + r * Math.cos(from) - ex, ny + r * Math.sin(from) - ey) < 1e-9);
  assert.ok(Math.hypot(nx + r * Math.cos(to) - mx, ny + r * Math.sin(to) - my) < 1e-9);
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
  // the default rate is 12 frames per second: frame 1 starts after 83.3 ms
  const ticks = [];
  const drawn = [];
  const defaults = new PetalStreamPlayer({ encoder, context, requestAnimationFrame: (callback) => ticks.push(callback), onFrame: (frame) => drawn.push(frame) });
  defaults.start();
  for (const time of [0, 83, 84, 167]) ticks.shift()(time);
  assert.deepEqual(drawn, [0, 1, 2]);
  defaults.stop();
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
  const counters = [];
  let clock = 0;
  const scanner = new PetalCameraScanner({
    video,
    createCanvas: (width, height) => Object.assign(canvas, { width, height }),
    onProgress: (outcome, stats) => {
      outcomes.push(outcome);
      counters.push(stats);
    },
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
  // the progress callback sees the session's counters: after the first frame the code is tracked
  assert.deepEqual(counters.at(-1), scanner.session.stats());
  assert.equal(counters[0].tracked, 0);
  assert.equal(counters.at(-1).tracked, counters.length - 1);
  assert.equal(counters.at(-1).inferred, 0);
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
        "  PETAL_LANES, PETAL_LAYOUT, PetalCameraScanner, PetalDecodedFrame, PetalError, PetalLuma, PetalPose,",
        "  PetalScanSession, PetalStreamEncoder, PetalStreamPlayer, decodePetalFrame, decodePetalFrameAt, drawPetalFrame,",
        "  encodeLane, fieldEvidence, mergeFields, petalDrawList, renderPetalFrame,",
        "  PETAL_TRACK_WINDOW_MS, locateCandidates, trackPetalFrame,",
        "  type PetalCompleted, type PetalDecodeErrorCode, type PetalFieldEvidence, type PetalLane, type PetalScanOutcome,",
        "  type PetalScanStats,",
        `} from ${JSON.stringify(declaration)};`,
        "declare const canvas: HTMLCanvasElement;",
        "declare const video: HTMLVideoElement;",
        "declare const stream: MediaStream;",
        "const context = canvas.getContext(\"2d\");",
        "if (context === null) throw new Error(\"no 2d context\");",
        "const encoder = new PetalStreamEncoder(new Uint8Array([1, 2, 3]), 2);",
        "const player = new PetalStreamPlayer({ encoder, context, fps: 8, cancelAnimationFrame: (id: number) => cancelAnimationFrame(id) });",
        "player.start();",
        "const scanner = new PetalCameraScanner({",
        "  video, stream, maxSide: 960, createCanvas: (width, height) => new OffscreenCanvas(width, height),",
        "  onProgress: (outcome: PetalScanOutcome, stats: PetalScanStats) => void (outcome.progress.rank + stats.tracked + stats.fieldLanes),",
        "  onComplete: (completed: PetalCompleted) => void completed.payload.byteLength,",
        "});",
        "void scanner.start();",
        "const image = renderPetalFrame(encoder.cells(0), { size: 256, supersample: 2 });",
        "const luma = PetalLuma.fromImageData(new ImageData(image.data, image.width, image.height));",
        "if (luma !== null) {",
        "  try {",
        "    const frame = decodePetalFrame(luma, { tryMirrored: false });",
        "    const lanes: number = frame.lanesOk();",
        "    const letters: string = frame.letters();",
        "    const corner: number | null = frame.inferredCorner;",
        "    const k: number = frame.pose.distortion;",
        "    const sector = frame.fields[3];",
        "    const lane: PetalLane = \"F2\";",
        "    const next = trackPetalFrame(luma, frame);",
        "    const at = decodePetalFrameAt(luma, PetalPose.plain(frame.pose.homography));",
        "    const evidence: PetalFieldEvidence | null = fieldEvidence(luma, frame);",
        "    const merged = evidence === null ? null : mergeFields([evidence], [false, true, false, false])[0];",
        "    const sets = locateCandidates(luma).filter((set) => set.inferred !== null).length;",
        "    void lanes; void letters; void corner; void k; void sector?.corrected; void frame.lane(lane);",
        "    void next?.inferredCorner; void at?.d; void merged?.erasures; void sets; void PETAL_TRACK_WINDOW_MS;",
        "  } catch (error) {",
        "    if (error instanceof PetalError) { const code: string = error.code; void code; }",
        "  }",
        "}",
        "const list = petalDrawList(encoder.cells(1));",
        "const petals: number = list.petals.length + list.backdrop.cells.length + list.petal.halfLength;",
        "drawPetalFrame(context, list, { size: 512 });",
        "const session = new PetalScanSession({ idleTimeoutMs: 10_000, decode: { maxPixels: 2_000_000 } });",
        "const outcome = session.push(new PetalLuma(64, 64), performance.now());",
        "const reason: PetalDecodeErrorCode | null = outcome.error;",
        "const fieldLen: number = PETAL_LANES.F0.wordLen + PETAL_LAYOUT.fieldCells;",
        "void petals; void reason; void fieldLen; void encodeLane(\"D\", new Uint8Array(19));",
        "// @ts-expect-error lanes are named D or F0-F3",
        "encodeLane(\"P\", new Uint8Array(19));",
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

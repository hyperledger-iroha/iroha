// Petal Stream format 5: the katakana bands of the card.
//
// Ports of the reference `glyphs`, `kana` and card `layout` unit tests and of the renderer's tile
// test, the band reader on clean cards, and the reader's intermediates on the golden captures.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { inflateSync } from "node:zlib";

import {
  PETAL_GLYPHS,
  PETAL_KANA,
  PETAL_LAYOUT,
  PETAL_PALETTE,
  PetalError,
  PetalFrameCells,
  PetalLuma,
  PetalStreamEncoder,
  PetalXorshift32,
  crc32c,
  decodeKanaLaneCounted,
  decodePetalFrame,
  encodeKanaLane,
  glyphStrokes,
  isGlyphInked,
  kanaCheck,
  kanaData,
  kanaDataTiles,
  kanaFirstId,
  kanaInked,
  kanaInside,
  kanaPacket,
  kanaPilotTiles,
  kanaTemplateProbe,
  kanaTileAt,
  kanaTiles,
  kanaWhitening,
  observeBand,
  petalDrawList,
  readKana,
  renderPetalFrame,
  trackPetalFrame,
  turnFirstId,
} from "../src/petal.js";
import { renderPetalSquare } from "../src/petal/render.js";

const hex = (bytes) => Buffer.from(bytes).toString("hex");
const LANES = PETAL_KANA.lanes;
const GRID = PETAL_KANA.grid;

function payload(len, seed) {
  const rng = new PetalXorshift32(seed);
  return Uint8Array.from({ length: len }, () => rng.nextByte());
}

function lumaOf(image) {
  return PetalLuma.fromRgba(image.width, image.height, image.data);
}

function ones(bytes) {
  let count = 0;
  for (const byte of bytes) for (let bit = 0; bit < 8; bit += 1) count += (byte >> bit) & 1;
  return count;
}

/** Cells whose dots and petals come from a fixed stream frame: the render tests' frame. */
function renderCells() {
  return new PetalStreamEncoder(payload(700, 2), 2).cells(4);
}

// ---------------------------------------------------------------- glyphs

/** Inked share of every cell of an 8 x 8 grid over the design square. */
function coverage(glyph) {
  const cell = PETAL_GLYPHS.grid / 8;
  return Array.from({ length: 64 }, (_, index) => {
    const u = index % 8;
    const v = Math.floor(index / 8);
    let inked = 0;
    for (let sy = 0; sy < 8; sy += 1) {
      for (let sx = 0; sx < 8; sx += 1) {
        if (isGlyphInked(glyph, (u + (sx + 0.5) / 8) * cell, (v + (sy + 0.5) / 8) * cell)) inked += 1;
      }
    }
    return inked / 64;
  });
}

test("every glyph has ink inside the design grid", () => {
  assert.equal(PETAL_GLYPHS.chars, "イロハニヘトワカレムノケアヒスン");
  assert.deepEqual([PETAL_GLYPHS.count, PETAL_GLYPHS.grid, PETAL_GLYPHS.strokeWidth], [16, 32, 6.5]);
  for (let glyph = 0; glyph < PETAL_GLYPHS.count; glyph += 1) {
    assert.ok(coverage(glyph).reduce((sum, value) => sum + value, 0) > 6, `glyph ${glyph}`);
    const strokes = glyphStrokes(glyph);
    assert.deepEqual(strokes, PETAL_GLYPHS.strokes[glyph]);
    for (const stroke of strokes) {
      assert.ok(stroke.length >= 2);
      for (const [x, y] of stroke) assert.ok(x >= 2 && x <= 30 && y >= 2 && y <= 30);
    }
  }
  // a point is inked within half a stroke width of a segment: イ's first stroke runs (22, 4)-(8, 18)
  assert.ok(isGlyphInked(0, 15, 11) && isGlyphInked(0, 22 + 3.25, 4) && !isGlyphInked(0, 22 + 3.26, 4));
  assert.ok(!isGlyphInked(1, 16, 16), "ロ is hollow");
  assert.throws(() => isGlyphInked(16, 0, 0), RangeError);
  assert.throws(() => glyphStrokes(-1), TypeError);
  assert.throws(() => isGlyphInked(0, "1", 0), TypeError);
});

test("glyphs are pairwise distinct under blur", () => {
  // zero-mean cosine distance of the coarse coverage must stay well apart
  const feature = (glyph) => {
    const values = coverage(glyph);
    const mean = values.reduce((sum, value) => sum + value, 0) / values.length;
    const centred = values.map((value) => value - mean);
    const norm = Math.sqrt(centred.reduce((sum, value) => sum + value * value, 0));
    return centred.map((value) => value / norm);
  };
  const features = Array.from({ length: 16 }, (_, glyph) => feature(glyph));
  for (let a = 0; a < 16; a += 1) {
    for (let b = a + 1; b < 16; b += 1) {
      const dot = features[a].reduce((sum, value, index) => sum + value * features[b][index], 0);
      assert.ok(1 - dot > 0.2, `glyphs ${a} and ${b}: ${1 - dot}`);
    }
  }
});

// ---------------------------------------------------------------- the card

test("the card holds 306 tiles per band and eight pilots each", () => {
  assert.deepEqual([PETAL_LAYOUT.kanaBandHeight, PETAL_LAYOUT.cardHeight, PETAL_LAYOUT.squareTop], [322, 1668, 322]);
  const tiles = kanaTiles();
  assert.equal(tiles.length, PETAL_LAYOUT.kanaTileCount);
  for (let band = 0; band < 2; band += 1) {
    const inBand = tiles.filter((tile) => tile.band === band);
    assert.equal(inBand.length, 306, `band ${band}`);
    assert.equal(inBand.filter((tile) => tile.pilot !== null).length, PETAL_LAYOUT.kanaPilots);
    assert.equal(kanaDataTiles(band).length, 306 - PETAL_LAYOUT.kanaPilots);
    kanaPilotTiles(band).forEach((tile, pilot) => {
      assert.equal(tiles[tile].pilot, pilot);
      assert.deepEqual([tiles[tile].row, tiles[tile].column], PETAL_LAYOUT.kanaPilotCells[band][pilot]);
    });
    // the data tiles are the band's other tiles, in reading order
    const data = kanaDataTiles(band);
    assert.deepEqual(data, tiles.flatMap((tile, index) => (tile.band === band && tile.pilot === null ? [index] : [])));
  }
  // the bands mirror each other about the square's middle
  for (const tile of tiles.filter((candidate) => candidate.band === 0)) {
    const mirror = tiles.find((t) => t.band === 1 && t.column === tile.column && t.row === 9 - tile.row);
    assert.ok(mirror !== undefined, "mirrored tile");
    assert.ok(Math.abs(mirror.y + 25 - 1024 + tile.y) < 1e-9);
    assert.equal(mirror.pilot, tile.pilot);
  }
  // no tile centre comes within the clearance of a finder centre
  for (const tile of tiles) {
    for (const [fx, fy] of PETAL_LAYOUT.finderCenters) {
      assert.ok(Math.hypot(tile.x + 12.5 - fx, tile.y + 12.5 - fy) >= PETAL_LAYOUT.kanaFinderClearance);
    }
  }
  // every tile centre maps back to its tile, and the square and the gutters hold none
  tiles.forEach((tile, index) => {
    assert.deepEqual(kanaTileAt(tile.x + 12.5, tile.y + 12.5), [index, 12.5, 12.5]);
  });
  assert.equal(kanaTileAt(512, 512), null);
  assert.equal(kanaTileAt(tiles[0].x - 1, tiles[0].y), null);
  assert.equal(kanaTileAt(Number.NaN, -100), null);
  assert.equal(kanaTileAt(512, -320), null, "above the lattice");
  // row 9 of band 0 has no tiles in columns 0 to 7, the cells too close to the top-left finder
  assert.equal(kanaTileAt(6.5 + 29 * 3 + 12.5, -312 + 29 * 9 + 12.5), null, "a cell too close to a finder");
  assert.deepEqual(
    tiles.filter((tile) => tile.band === 0 && tile.row === 9).map((tile) => tile.column).slice(0, 2),
    [8, 9],
  );
  assert.deepEqual([tiles[0].x, tiles[0].y, tiles[tiles.length - 1].y], [6.5, -312, 1050 + 29 * 9]);
  assert.throws(() => kanaDataTiles(2), RangeError);
  assert.throws(() => kanaTileAt("1", 0), TypeError);
});

test("a tile is a rounded square and a glyph inks inside its box", () => {
  assert.ok(kanaInside(0, 12.5) && kanaInside(12.5, 25) && kanaInside(3, 0));
  assert.ok(!kanaInside(0, 0) && !kanaInside(25, 25) && !kanaInside(-0.1, 12) && !kanaInside(12, Number.NaN));
  // the corner arc: within 3 of (3, 3)
  assert.ok(kanaInside(3 - 3 * Math.SQRT1_2 + 1e-9, 3 - 3 * Math.SQRT1_2 + 1e-9) && !kanaInside(0.5, 0.5));
  // the glyph box is the tile inset by 1: design point ((u - 1) * 32 / 23, (v - 1) * 32 / 23)
  for (let glyph = 0; glyph < 16; glyph += 1) {
    for (const [u, v] of [
      [4, 4],
      [12.5, 12.5],
      [20, 9],
    ]) {
      assert.equal(kanaInked(glyph, u, v), isGlyphInked(glyph, ((u - 1) * 32) / 23, ((v - 1) * 32) / 23));
    }
  }
  assert.throws(() => kanaInked(16, 1, 1), RangeError);
});

test("every glyph stroke lies inside its rounded tile", () => {
  // what lets a canvas stroke the glyphs without clipping them to the tile
  for (let glyph = 0; glyph < 16; glyph += 1) {
    for (let u = -1; u <= 26; u += 0.25) {
      for (let v = -1; v <= 26; v += 0.25) {
        if (isGlyphInked(glyph, ((u - 1) * 32) / 23, ((v - 1) * 32) / 23)) {
          assert.ok(kanaInside(u, v), `glyph ${glyph} inked outside its tile at (${u}, ${v})`);
        }
      }
    }
  }
  const list = petalDrawList(new PetalFrameCells());
  assert.equal(list.tile.strokeWidth, (PETAL_GLYPHS.strokeWidth * 23) / 32);
  assert.equal(list.tile.glyphs[0][0][0][0], 1 + (22 * 23) / 32);
});

// ---------------------------------------------------------------- kana lanes

test("kana lane sizes and ids are consistent", () => {
  assert.deepEqual([PETAL_KANA.glyph.wordLen, PETAL_KANA.polarity.wordLen], [149, 37]);
  assert.equal(kanaTiles().length, 612);
  assert.equal(kanaDataTiles(0).length, PETAL_KANA.dataTiles);
  const ids = new Set();
  for (const frame of [0, 1, 65535]) {
    LANES.forEach((lane, index) => {
      const atoms = index % 2 === 0 ? PETAL_KANA.polarity.atoms : PETAL_KANA.glyph.atoms;
      for (let k = 0; k < atoms; k += 1) {
        const id = kanaFirstId(frame, lane) + k;
        assert.ok(!ids.has(id), `${lane} frame ${frame} atom ${k}`);
        ids.add(id);
      }
    });
  }
  // above every turn atom id and below the u32 limit
  assert.ok(kanaFirstId(0, "K0") >= turnFirstId(65535, 3) + 4);
  assert.ok(kanaFirstId(65535, "P1") < 2 ** 32 - 1 - 16);
  assert.deepEqual(
    LANES.map((lane) => kanaFirstId(1, lane)),
    [2 ** 31 + 2 ** 20 + 16 + 7, 2 ** 31 + 2 ** 20 + 16, 2 ** 31 + 2 ** 20 + 16 + 8 + 7, 2 ** 31 + 2 ** 20 + 16 + 8],
  );
  // the frame counter wraps
  assert.equal(kanaFirstId(65536 + 7, "K1"), kanaFirstId(7, "K1"));
  // the check covers the tag, the frame (big-endian), the lane's code (0x10 | band << 1 | kind) and the atoms
  const atoms = Uint8Array.from({ length: 16 }, (_, i) => i * 3);
  LANES.forEach((lane, index) => {
    const code = 0x10 | ((index >> 1) << 1) | (index & 1);
    assert.equal(kanaCheck({ tag: 0x5a, frame: 0x0102 }, lane, atoms), crc32c([0x5a, 0x01, 0x02, code, ...atoms]), lane);
  });
  assert.throws(() => kanaFirstId(0, "P2"), TypeError);
  assert.throws(() => kanaFirstId(0.5, "P0"), TypeError);
  assert.throws(() => kanaCheck({ tag: 256, frame: 0 }, "P0", atoms), TypeError);
});

test("kana whitening is balanced and distinct per lane", () => {
  const all = LANES.map((lane) => kanaWhitening(lane));
  all.forEach((whitening, index) => {
    assert.equal(whitening.length, index % 2 === 0 ? 37 : 149);
    const bits = whitening.length * 8;
    const count = ones(whitening);
    assert.ok(count > Math.floor((bits * 35) / 100) && count < Math.floor((bits * 65) / 100), `${LANES[index]} ones ${count}/${bits}`);
  });
  assert.notDeepEqual(all[1].subarray(0, 37), all[0]);
  assert.notDeepEqual(all[1], all[3]);
  // seeds "POL0", "KAN1"
  const rng = new PetalXorshift32(0x504f4c30);
  assert.deepEqual(all[0], Uint8Array.from({ length: 37 }, () => rng.nextByte()));
  const glyphs = new PetalXorshift32(0x4b414e31);
  assert.deepEqual(all[3], Uint8Array.from({ length: 149 }, () => glyphs.nextByte()));
  assert.throws(() => kanaWhitening("K2"), TypeError);
});

test("kana symbols roundtrip through codewords", () => {
  const header = { tag: 0x5a, frame: 77 };
  const words = [];
  const atoms = [];
  LANES.forEach((lane, index) => {
    const band = index >> 1;
    const laneAtoms =
      index % 2 === 0
        ? Uint8Array.from({ length: 16 }, (_, i) => i ^ band)
        : Uint8Array.from({ length: 112 }, (_, i) => ((i * 7) & 0xff) ^ band);
    atoms.push(laneAtoms);
    words.push(encodeKanaLane(lane, kanaData(header, lane, laneAtoms)));
  });
  const cells = new PetalFrameCells();
  cells.setKana(words);
  for (let band = 0; band < 2; band += 1) {
    const [polarity, glyph] = [LANES[2 * band], LANES[2 * band + 1]];
    assert.deepEqual(cells.kanaPolarityWord(band), words[2 * band]);
    assert.deepEqual(cells.kanaGlyphWord(band), words[2 * band + 1]);
    const { data, corrected } = decodeKanaLaneCounted(glyph, words[2 * band + 1]);
    assert.equal(corrected, 0);
    const packet = kanaPacket(header, glyph, data);
    assert.ok(packet !== null, "check matches");
    assert.equal(packet.firstId, kanaFirstId(77, glyph));
    assert.deepEqual(Buffer.concat(packet.atoms), Buffer.from(atoms[2 * band + 1]));
    // the check refuses another frame, band or kind
    assert.equal(kanaPacket({ tag: 0x5a, frame: 78 }, glyph, data), null);
    assert.equal(kanaPacket({ tag: 0x5b, frame: 77 }, glyph, data), null);
    assert.equal(kanaPacket(header, LANES[2 * (1 - band) + 1], data), null);
    const polarityData = decodeKanaLaneCounted(polarity, words[2 * band]).data;
    assert.deepEqual(kanaPacket(header, polarity, polarityData).atoms[0], atoms[2 * band]);
    assert.equal(kanaPacket(header, glyph, polarityData), null, "a polarity lane's data has the wrong length for a glyph lane");
  }
  // pilots keep their symbols; the data tiles beyond the polarity codeword are light
  kanaTiles().forEach((tile, index) => {
    if (tile.pilot === null) return;
    const [glyph, light] = PETAL_LAYOUT.kanaPilotSymbols[tile.pilot];
    assert.deepEqual([cells.kanaGlyphs[index], cells.kanaLight[index]], [glyph, light ? 1 : 0], `tile ${index}`);
  });
  for (let band = 0; band < 2; band += 1) {
    const data = kanaDataTiles(band);
    assert.ok(cells.kanaLight[data[296]] === 1 && cells.kanaLight[data[297]] === 1);
  }
  assert.ok(cells.equals(new PetalFrameCells(cells.dots, cells.petals, cells.turns, cells.kanaGlyphs, cells.kanaLight)));
  assert.ok(!cells.equals(new PetalFrameCells(cells.dots, cells.petals, cells.turns)));
  // damaged words decode within the parity, the check catches the rest
  const damaged = words[1].slice();
  for (let position = 0; position < 16; position += 1) damaged[position * 9] ^= 0xa5;
  assert.deepEqual(decodeKanaLaneCounted("K0", damaged).data, decodeKanaLaneCounted("K0", words[1]).data);
  assert.throws(() => decodeKanaLaneCounted("K0", new Uint8Array(149).fill(0x55)), (error) => error instanceof PetalError && error.code === "rs_uncorrectable");
  assert.throws(() => decodeKanaLaneCounted("P0", new Uint8Array(36)), (error) => error instanceof PetalError && error.code === "rs_invalid_shape");
  assert.throws(() => kanaData(header, "P0", new Uint8Array(17)), RangeError);
  assert.throws(() => encodeKanaLane("K1", new Uint8Array(115)), RangeError);
  assert.throws(() => cells.setKana(words.slice(0, 3)), TypeError);
  assert.throws(() => cells.setKana([words[1], words[0], words[2], words[3]]), RangeError);
  assert.throws(() => cells.kanaPolarityWord(2), RangeError);
  assert.throws(() => new PetalFrameCells(undefined, undefined, undefined, new Uint8Array(612).fill(16)), RangeError);
  assert.throws(() => new PetalFrameCells(undefined, undefined, undefined, undefined, new Uint8Array(611)), RangeError);
});

test("kana templates are unit vectors that tell the glyphs apart", () => {
  for (let blur = 0; blur < PETAL_KANA.blurs.length; blur += 1) {
    for (let glyph = 0; glyph < 16; glyph += 1) {
      for (const light of [true, false]) {
        let norm = 0;
        for (let p = 0; p < GRID * GRID; p += 1) norm += kanaTemplateProbe(blur, glyph, light, p % GRID, Math.floor(p / GRID)) ** 2;
        assert.ok(Math.abs(norm - 1) < 1e-9, `blur ${blur} glyph ${glyph} light ${light}`);
      }
    }
  }
  // at the sharpest blur no two light templates correlate above 0.8
  for (let a = 0; a < 16; a += 1) {
    for (let b = a + 1; b < 16; b += 1) {
      let dot = 0;
      for (let p = 0; p < GRID * GRID; p += 1) {
        dot += kanaTemplateProbe(0, a, true, p % GRID, Math.floor(p / GRID)) * kanaTemplateProbe(0, b, true, p % GRID, Math.floor(p / GRID));
      }
      assert.ok(dot < 0.8, `glyphs ${a} and ${b}: ${dot}`);
    }
  }
  assert.throws(() => kanaTemplateProbe(5, 0, true, 0, 0), RangeError);
  assert.throws(() => kanaTemplateProbe(0, 0, true, 12, 0), RangeError);
});

// ---------------------------------------------------------------- rendering

test("katakana tiles show their polarity and glyph", () => {
  const base = renderCells();
  const glyphs = Uint8Array.from({ length: 612 }, (_, tile) => tile % 16);
  const light = Uint8Array.from({ length: 612 }, (_, tile) => (tile % 3 !== 0 ? 1 : 0));
  const frame = new PetalFrameCells(base.dots, base.petals, base.turns, glyphs, light);
  const image = renderPetalFrame(frame, { size: 1024, supersample: 1 });
  assert.deepEqual([image.width, image.height], [1024, 1668]);
  const palette = PETAL_PALETTE;
  const top = PETAL_LAYOUT.squareTop;
  kanaTiles().forEach((tile, index) => {
    // the pixel centres of the tile's square, against the definition
    const x0 = Math.ceil(tile.x);
    const y0 = Math.ceil(tile.y + top);
    for (const [dx, dy] of [
      [3, 3],
      [12, 12],
      [8, 17],
      [20, 6],
    ]) {
      const px = x0 + dx;
      const py = y0 + dy;
      const inked = kanaInked(glyphs[index], px + 0.5 - tile.x, py + 0.5 - top - tile.y);
      let expected;
      if (light[index] === 1) expected = inked ? palette.ink : palette.light;
      else expected = inked ? palette.pink : palette.background;
      const at = (py * 1024 + px) * 4;
      assert.deepEqual(Array.from(image.data.subarray(at, at + 3)), expected, `tile ${index} at (${dx}, ${dy})`);
    }
  });
  // the gutters and the margins are background
  for (const [x, y] of [
    [4, 10],
    [33.5 + 0.2, 40],
    [512, top - 10],
    [512, top + 1024 + 10],
  ]) {
    const at = (Math.floor(y) * 1024 + Math.floor(x)) * 4;
    assert.deepEqual(Array.from(image.data.subarray(at, at + 3)), palette.background, `(${x}, ${y})`);
  }
  // the palette's ink is used for the glyphs on light tiles only
  const inked = renderPetalFrame(frame, { size: 256, supersample: 1, palette: { ink: [1, 2, 3] } });
  assert.ok(inked.data.some((_, index) => index % 4 === 0 && inked.data[index] === 1 && inked.data[index + 1] === 2));
  assert.throws(() => renderPetalFrame(frame, { palette: { ink: [1, 2] } }), TypeError);
});

// ---------------------------------------------------------------- the band reader

/** The kana reader tests' stream: 3,000 bytes from xorshift32 seed 3, kind 2. */
function kanaStream() {
  return new PetalStreamEncoder(payload(3000, 3), 2);
}

test("a clean card reads all four kana lanes and skips what it is told", () => {
  const encoder = kanaStream();
  const cells = encoder.cells(5);
  const image = lumaOf(renderPetalFrame(cells, { size: 768, supersample: 2 }));
  const decoded = decodePetalFrame(image);
  assert.equal(decoded.letters(), "D0123");
  const header = { tag: encoder.meta.tag, frame: 5 };
  const truth = encoder.laneData(5).kana;
  const results = readKana(image, decoded, header);
  results.forEach((result, index) => {
    assert.ok(result !== null, LANES[index]);
    assert.deepEqual([hex(result.data), result.corrected, result.erasures, result.header], [hex(truth[index]), 0, 0, header]);
  });
  assert.deepEqual(
    readKana(image, decoded, header, [true, false]).map((result) => result !== null),
    [false, false, true, true],
  );
  assert.deepEqual(readKana(image, decoded, header, [true, true]), [null, null, null, null]);
  // the reader sees every data tile as drawn, under every pilot
  for (let band = 0; band < 2; band += 1) {
    const reading = observeBand(image, decoded, band);
    assert.ok(reading !== null);
    assert.equal(reading.pilots, 8);
    assert.ok(PETAL_KANA.blurs.includes(reading.blur));
    assert.equal(reading.tiles.length, PETAL_KANA.dataTiles);
    reading.tiles.forEach((tile, i) => {
      assert.ok(tile !== null, `band ${band} tile ${i}`);
      assert.equal(tile.tile, kanaDataTiles(band)[i]);
      assert.deepEqual([tile.glyph, tile.light ? 1 : 0], [cells.kanaGlyphs[tile.tile], cells.kanaLight[tile.tile]]);
      // a light tile with much ink (ロ) is darker than one with little, so margins stay below 1
      assert.ok(tile.lightMargin > 0.2 && tile.glyphLead > 0.2, `band ${band} tile ${i}`);
    });
  }
  // the session's header comes from the square: a header naming another stream reads nothing
  assert.deepEqual(readKana(image, decoded, { tag: (header.tag + 1) & 0xff, frame: 5 }), [null, null, null, null]);
});

test("a band of the frame before or after the square is read under its own frame", () => {
  const encoder = kanaStream();
  const header = { tag: encoder.meta.tag, frame: 8 };
  for (const [shown, read] of [
    [9, true],
    [7, true],
    [10, false],
  ]) {
    // the square shows frame 8, the bands frame `shown` (a rolling shutter caught the change)
    const cells = encoder.cells(8);
    cells.setKana(encoder.words(shown).kana);
    const image = lumaOf(renderPetalFrame(cells, { size: 768, supersample: 2 }));
    const decoded = decodePetalFrame(image);
    assert.equal(decoded.atomPackets()[0].header.frame, 8);
    const results = readKana(image, decoded, header);
    const truth = encoder.laneData(shown).kana;
    results.forEach((result, index) => {
      if (!read) {
        assert.equal(result, null, `frame ${shown} ${LANES[index]}`);
        return;
      }
      assert.deepEqual([hex(result.data), result.header.frame], [hex(truth[index]), shown], `frame ${shown} ${LANES[index]}`);
      // its atoms carry the ids of the frame the check matched
      assert.equal(kanaPacket(result.header, LANES[index], result.data).firstId, kanaFirstId(shown, LANES[index]));
    });
  }
  // the frame counter wraps: frame 0's neighbours are 65535 and 1
  const wrap = encoder.cells(0);
  wrap.setKana(encoder.words(65535).kana);
  const image = lumaOf(renderPetalFrame(wrap, { size: 768, supersample: 2 }));
  const results = readKana(image, decodePetalFrame(image), { tag: encoder.meta.tag, frame: 0 });
  assert.ok(results.every((result) => result !== null && result.header.frame === 65535));
});

test("a bare square has no bands to read, and the reader refuses what it cannot sample", () => {
  const encoder = kanaStream();
  const image = lumaOf(renderPetalSquare(encoder.cells(5), { size: 768, supersample: 2 }));
  const decoded = decodePetalFrame(image);
  const header = { tag: encoder.meta.tag, frame: 5 };
  assert.deepEqual(readKana(image, decoded, header), [null, null, null, null]);
  assert.equal(observeBand(image, decoded, 0), null);
  // an empty or inconsistent plane is read as nothing
  assert.deepEqual(readKana(new PetalLuma(0, 0), decoded, header), [null, null, null, null]);
  assert.deepEqual(readKana({ width: 10, height: 10, data: new Uint8Array(5) }, decoded, header), [null, null, null, null]);
  assert.equal(observeBand(new PetalLuma(0, 0), decoded, 1), null);
  // malformed arguments
  assert.throws(() => readKana(image, { pose: null }, header), TypeError);
  assert.throws(() => readKana({ width: 10, height: 10, data: [] }, decoded, header), TypeError);
  assert.throws(() => readKana(image, decoded, { tag: 1, frame: 65536 }), TypeError);
  assert.throws(() => readKana(image, decoded, header, [false]), TypeError);
  assert.throws(() => observeBand(image, decoded, 2), RangeError);
});

test("a tracked capture reads the bands under the tracked pose", () => {
  const encoder = kanaStream();
  const card = (frame) => lumaOf(renderPetalFrame(encoder.cells(frame), { size: 768, supersample: 2 }));
  const first = decodePetalFrame(card(5));
  const next = card(6);
  const followed = trackPetalFrame(next, first);
  assert.ok(followed !== null);
  const results = readKana(next, followed, { tag: encoder.meta.tag, frame: 6 });
  assert.deepEqual(results.map((result) => hex(result.data)), encoder.laneData(6).kana.map(hex));
});

// ---------------------------------------------------------------- golden captures

const captureFixture = JSON.parse(readFileSync(new URL("../../../fixtures/petal/petal_captures_v5.json", import.meta.url), "utf8"));

function captureLuma(entry, key = "luma_zlib_base64") {
  return new PetalLuma(entry.width, entry.height, new Uint8Array(inflateSync(Buffer.from(entry[key], "base64"))));
}

/** The header a decoded capture's lanes name: its first atom packet's, or its beacon's. */
function headerOf(frame) {
  return frame.atomPackets()[0]?.header ?? frame.beacon().header;
}

/**
 * What the reference band reader (`read_kana` with no band passed over, under the header the
 * capture's lanes name) reads from the golden captures: corrected/erasures per kana lane, and per
 * band what `observe_band` saw: the blur it chose (an index into `PETAL_KANA.blurs`), the pilots it
 * found and the data tiles it read.
 */
const REFERENCE_KANA = Object.freeze({
  "clean-512": ["P0:0/0 K0:0/0 P1:0/0 K1:0/0", "0:0/8/298 1:0/8/298"],
  "modern-720p-rotated": ["P0:- K0:- P1:3/0 K1:7/0", "0:1/6/234 1:2/7/286"],
  "legacy-540p-tilted": ["P0:- K0:- P1:8/0 K1:-", "0:3/5/188 1:3/6/255"],
  "soft-480p-blur1.9": ["P0:- K0:- P1:- K1:-", "0:4/5/285 1:4/4/245"],
  "small-480p": ["P0:2/0 K0:- P1:1/0 K1:-", "0:4/6/294 1:4/6/294"],
  "selfie-mirrored-540p": ["P0:- K0:- P1:- K1:-", "0:2/5/145 1:2/4/110"],
  "overexposed-540p": ["P0:- K0:- P1:12/8 K1:-", "0:2/5/207 1:2/6/250"],
  "veiled-720p": ["P0:6/0 K0:16/0 P1:0/0 K1:9/0", "0:2/6/283 1:2/8/298"],
  "shadow-band-540p": ["P0:- K0:- P1:- K1:-", "0:1/3/44 1:1/3/44"],
  "hidden-corner-540p": ["P0:- K0:- P1:- K1:-", "0:2/5/146 1:2/5/184"],
  "cut-corner-720p": ["P0:1/0 K0:1/0 P1:- K1:-", "0:1/7/296 1:1/4/179"],
  "barrel-480p": ["P0:- K0:- P1:- K1:-", "0:3/4/191 1:3/5/224"],
  "upright-720p": ["P0:0/0 K0:0/0 P1:0/0 K1:0/0", "0:1/8/298 1:1/8/298"],
  "upright-legacy-720p": ["P0:0/0 K0:15/0 P1:0/0 K1:10/0", "0:2/8/298 1:2/8/298"],
  "upright-soft-720p": ["P0:0/0 K0:0/0 P1:2/0 K1:6/0", "0:3/8/298 1:3/6/290"],
  "upright-480p": ["P0:12/8 K0:- P1:0/0 K1:0/0", "0:3/5/205 1:3/8/298"],
  "steady-hand-540p": ["P0:- K0:- P1:- K1:-", "0:2/6/219 1:2/6/243"],
  "thumb-arrives-540p": ["P0:- K0:- P1:- K1:-", "0:2/5/214 1:2/6/225"],
});

function assertReading(name, image, frame, entry) {
  const results = readKana(image, frame, headerOf(frame));
  const counts = results.map((result, index) => `${LANES[index]}:${result === null ? "-" : `${result.corrected}/${result.erasures}`}`).join(" ");
  const bands = [0, 1]
    .map((band) => {
      const reading = observeBand(image, frame, band);
      if (reading === null) return `${band}:-`;
      return `${band}:${PETAL_KANA.blurs.indexOf(reading.blur)}/${reading.pilots}/${reading.tiles.filter((tile) => tile !== null).length}`;
    })
    .join(" ");
  assert.deepEqual([counts, bands], REFERENCE_KANA[name], `${name}: kana lanes and bands`);
  results.forEach((result, index) => {
    if (result !== null) assert.equal(hex(result.data), entry.kana_data[index], `${name}: ${LANES[index]} data`);
  });
}

test("the band reader matches the reference's intermediates on the golden captures", () => {
  assert.equal(captureFixture.captures.length, 16);
  for (const capture of captureFixture.captures) {
    const image = captureLuma(capture);
    assertReading(capture.name, image, decodePetalFrame(image), capture);
  }
  for (const pair of captureFixture.tracks) {
    const previous = decodePetalFrame(captureLuma(pair, "from_luma_zlib_base64"));
    const image = captureLuma(pair, "to_luma_zlib_base64");
    assertReading(pair.name, image, trackPetalFrame(image, previous), pair);
  }
});

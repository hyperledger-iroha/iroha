import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { inflateSync } from "node:zlib";

import {
  PetalDecodedFrame,
  PetalLuma,
  PetalScanSession,
  PetalStreamAssembler,
  PetalStreamEncoder,
  PetalXorshift32,
  decodePetalFrame,
  fieldEvidence,
  mergeFields,
  renderPetalFrame,
} from "../src/petal.js";
import { MERGE_CAPTURES, MERGE_WINDOW } from "../src/petal/session.js";

const fixture = JSON.parse(readFileSync(new URL("../../../fixtures/petal/petal_captures_v5.json", import.meta.url)));
const hex = (bytes) => Buffer.from(bytes).toString("hex");
const imageOf = (set, capture) => new PetalLuma(set.width, set.height,
  new Uint8Array(inflateSync(Buffer.from(capture.luma_zlib_base64, "base64"))));
/** The sector digits of the field lanes that read. */
const letters = (fields) => fields.map((field, sector) => (field === null ? "" : String(sector))).join("");
/** Corrected and erased bytes of every field lane that read. */
const counts = (fields) =>
  fields.map((field, sector) => `${sector}:${field === null ? "-" : `${field.corrected}/${field.erasures}`}`).join(" ");
const NO_FIELDS = Object.freeze([null, null, null, null]);
/** The four-capture merge of each set as the reference reads it (`merge_fields`). */
const REFERENCE_MERGES = Object.freeze({
  "noisy-480p": "0:5/0 1:10/0 2:- 3:11/0",
  "mixed-frames-480p": "0:7/0 1:9/0 2:- 3:7/0",
});
/** The lanes a reference scan session merges from the noisy set, in the order it offers them. */
const REFERENCE_SESSION_MERGES = Object.freeze({ "noisy-480p": "013" });

test("petal averaging: the fixture holds one merge set and one mixed set", () => {
  assert.deepEqual(fixture.merges.map((set) => [set.name, set.must_merge, set.reference_merged]), [["noisy-480p", "01", "013"]]);
  assert.deepEqual(fixture.mixed_merges.map((set) => [set.name, set.reference_merged]), [["mixed-frames-480p", "013"]]);
});

for (const set of fixture.merges) {
  test(`petal averaging: ${set.name} recovers the recorded field lanes`, () => {
    const evidence = [];
    for (const capture of set.captures) {
      const image = imageOf(set, capture);
      const frame = decodePetalFrame(image);
      assert.equal(frame.letters(), capture.reference_decoded);
      for (const letter of capture.must_decode) assert.ok(frame.letters().includes(letter));
      for (const letter of set.must_merge) assert.equal(frame.fields[Number(letter)], null, `lane ${letter} alone`);
      evidence.push(fieldEvidence(image, frame));
    }
    const recovered = mergeFields(evidence);
    assert.equal(letters(recovered), set.reference_merged);
    assert.equal(counts(recovered), REFERENCE_MERGES[set.name]);
    recovered.forEach((result, sector) => {
      if (result !== null) assert.equal(hex(result.data), set.field_data[sector], `merged lane ${sector}`);
    });
  });

  test(`petal session: ${set.name} adds only newly recovered lanes once`, () => {
    const session = new PetalScanSession();
    let recovered = "";
    for (const [i, capture] of set.captures.entries()) {
      const outcome = session.push(imageOf(set, capture), 33 * i);
      assert.equal(outcome.error, null);
      assert.equal(outcome.lanes, capture.reference_decoded);
      recovered += outcome.merged;
    }
    for (const letter of set.must_merge) assert.equal(recovered.split(letter).length - 1, 1, `lane ${letter}`);
    assert.equal(new Set(recovered).size, recovered.length, "every lane is recovered once");
    // merging two, then three, then four held captures, as the reference session does
    assert.equal(recovered, REFERENCE_SESSION_MERGES[set.name]);
    const stats = session.stats();
    assert.equal(stats.mergedLanes, recovered.length);
    assert.equal(stats.fieldLanes, 0, "no capture reads a field lane alone");
    assert.equal(stats.merges, 3);
    // merged lanes are not single-capture reads: no turn lane rides on them; the bands are tried
    // once for the shown frame, on its first capture
    assert.equal(stats.turnLanes, 0);
    assert.equal(stats.kanaTries, 2);
    // the merged atoms wait for the beacon of their stream
    assert.equal(session.progress().atomsReceived, 0);
  });
}

test("petal averaging: mixed-frame field lanes never enter the assembler", () => {
  for (const set of fixture.mixed_merges) {
    const session = new PetalScanSession();
    const baseline = new PetalStreamAssembler();
    const evidence = [];
    for (const [i, capture] of set.captures.entries()) {
      const image = imageOf(set, capture);
      const frame = decodePetalFrame(image);
      assert.equal(frame.letters(), capture.reference_decoded);
      frame.feed(baseline);
      evidence.push(fieldEvidence(image, frame));
      assert.equal(session.push(image, 33 * i).merged, "");
    }
    // the merge reads the petal field, which shows another frame than the rings
    const recovered = mergeFields(evidence);
    assert.equal(letters(recovered), set.reference_merged);
    assert.equal(counts(recovered), REFERENCE_MERGES[set.name]);
    recovered.forEach((result, sector) => {
      if (result === null) return;
      assert.equal((result.data[1] << 8) | result.data[2], set.petals_frame);
      assert.equal(hex(result.data), set.field_data[sector]);
    });
    assert.notEqual(set.petals_frame, set.frame);
    assert.deepEqual(session.progress(), baseline.progress());
    assert.equal(session.stats().mergedLanes, 0);
    assert.ok(session.stats().merges > 0);
  }
});

test("petal evidence: one capture matches normal decoding and rejects unusable inputs", () => {
  for (const name of ["clean-512", "shadow-band-540p", "hidden-corner-540p", "selfie-mirrored-540p", "barrel-480p"]) {
    const capture = fixture.captures.find((c) => c.name === name);
    const image = imageOf(capture, capture);
    const frame = decodePetalFrame(image);
    const evidence = fieldEvidence(image, frame);
    assert.deepEqual(mergeFields([evidence]), frame.fields, name);
    assert.equal(fieldEvidence(new PetalLuma(20, 20), frame), null);
    const invalid = new PetalDecodedFrame(frame.pose, frame.rotation, frame.mirrored, frame.d, frame.fields, 7);
    assert.equal(fieldEvidence(image, invalid), null);
  }
  assert.deepEqual(mergeFields([]), [null, null, null, null]);
});

/** The `one-pass` stream of the golden captures. */
function onePass() {
  const rng = new PetalXorshift32(2);
  return new PetalStreamEncoder(Uint8Array.from({ length: 3000 }, () => rng.nextByte()), 2);
}

function render(encoder, frame, supersample = 2) {
  const rendered = renderPetalFrame(encoder.cells(frame), { size: 512, supersample });
  return PetalLuma.fromRgba(rendered.width, rendered.height, rendered.data);
}

const exact = (bytes) => ({ data: bytes, corrected: 0, erasures: 0 });

/**
 * A clean render of frame `petals` and its decoded frame with the lanes replaced by lane D of
 * frame `rings`, as if the rings showed another frame than the petal field.
 */
function ringsOfAnotherFrame(encoder, petals, rings) {
  const image = render(encoder, petals);
  const frame = decodePetalFrame(image);
  assert.ok(frame.fields.every((field) => field !== null));
  return {
    image,
    frame: new PetalDecodedFrame(frame.pose, frame.rotation, frame.mirrored, exact(encoder.laneData(rings).d), NO_FIELDS, null),
  };
}

test("petal session: a merge never offers atoms of another shown frame", () => {
  const encoder = onePass();
  const session = new PetalScanSession();
  const { image, frame } = ringsOfAnotherFrame(encoder, 11, 10);
  for (let capture = 0; capture < 3; capture += 1) assert.equal(session._merge(image, frame), "", `capture ${capture}`);
  const stats = session.stats();
  assert.deepEqual([stats.merges, stats.mergedLanes], [2, 0]);
  assert.equal(session.progress().atomsReceived, 0);
  // what was refused: the merged field lanes name frame 11, not frame 10
  const held = session._held[0];
  assert.equal(held.shown.frame, 10);
  const merged = mergeFields(held.evidence);
  const truth = encoder.laneData(11);
  merged.forEach((result, sector) => assert.deepEqual(result?.data, truth.fields[sector]));
});

test("petal session: torn captures whose lanes name different frames are not held", () => {
  const encoder = onePass();
  const session = new PetalScanSession();
  const { image, frame } = ringsOfAnotherFrame(encoder, 11, 10);
  const torn = new PetalDecodedFrame(frame.pose, frame.rotation, frame.mirrored, frame.d,
    [null, null, exact(encoder.laneData(11).fields[2]), null], null);
  assert.equal(session._merge(image, torn), "");
  assert.equal(session._held.length, 0);
  const noLanes = new PetalDecodedFrame(frame.pose, frame.rotation, frame.mirrored, null, NO_FIELDS, null);
  assert.equal(session._merge(image, noLanes), "");
  assert.equal(session._held.length, 0);
});

test("petal session: the session holds at most four captures of six shown frames", () => {
  const encoder = onePass();
  const session = new PetalScanSession();
  // refused merges never finish a shown frame, so its captures stay held
  const { image, frame } = ringsOfAnotherFrame(encoder, 11, 10);
  for (let capture = 0; capture < MERGE_CAPTURES + 2; capture += 1) session._merge(image, frame);
  assert.equal(session._held.length, 1);
  assert.equal(session._held[0].evidence.length, MERGE_CAPTURES);
  assert.equal(session.stats().merges, MERGE_CAPTURES + 1);
  for (let rings = 12; rings < 12 + MERGE_WINDOW; rings += 1) {
    const other = new PetalDecodedFrame(frame.pose, frame.rotation, frame.mirrored, exact(encoder.laneData(rings).d), NO_FIELDS, null);
    session._merge(image, other);
  }
  assert.equal(session._held.length, MERGE_WINDOW);
  assert.ok(session._held.every((held) => held.shown.frame >= 12 && held.evidence.length <= MERGE_CAPTURES));
  session.reset();
  assert.equal(session._held.length, 0);
});

test("petal session: a stream switch with the same short tag discards old merge state", () => {
  const encoder = (last) => {
    const bytes = new Uint8Array(100);
    bytes[3] = last;
    return new PetalStreamEncoder(bytes, 2);
  };
  const old = encoder(49);
  const fresh = encoder(128);
  assert.deepEqual([old.meta.tag, fresh.meta.tag], [61, 61]);
  assert.notDeepEqual(old.meta, fresh.meta);
  const oldImage = render(old, 0, 1);
  const newImage = render(fresh, 0, 1);
  const oldFull = decodePetalFrame(oldImage);
  const partial = (frame) => new PetalDecodedFrame(frame.pose, frame.rotation, frame.mirrored, frame.d, NO_FIELDS, null);
  const newPartial = partial(decodePetalFrame(newImage));
  for (const wasComplete of [false, true]) {
    const session = new PetalScanSession();
    const previous = wasComplete ? oldFull : partial(oldFull);
    session._absorb(previous);
    session._merge(oldImage, previous);
    assert.ok(session._held.length > 0);
    session._tried.push({ tag: old.meta.tag, frame: 0, sector: 1 });
    session._absorb(newPartial);
    assert.deepEqual(session.progress().meta, old.meta, "one conflicting beacon cannot switch streams");
    assert.equal(session._tried.length, 1);
    session._absorb(newPartial);
    assert.deepEqual(session.progress().meta, fresh.meta);
    assert.equal(session._held.length, 0, "an accepted stream switch discards held evidence");
    assert.equal(session._tried.length, 0, "and forgets the turn lanes tried, which share the short tag");
    assert.equal(session._merge(newImage, newPartial), "");
    assert.equal(session._held[0].evidence.length, 1);
    assert.ok(session._held[0].done.every((done) => !done));
    assert.equal(session._merge(newImage, newPartial), "0123");
  }
});

test("petal session: the first beacon preserves evidence received before it", () => {
  const encoder = onePass();
  const image = render(encoder, 1);
  const decoded = decodePetalFrame(image);
  const partial = new PetalDecodedFrame(decoded.pose, decoded.rotation, decoded.mirrored, decoded.d, NO_FIELDS, null);
  const session = new PetalScanSession();
  session._absorb(partial);
  session._merge(image, partial);
  assert.equal(session.progress().meta, null);
  assert.equal(session._held[0].evidence.length, 1);
  const evidence = session._held[0].evidence[0];
  const beaconDecoded = decodePetalFrame(render(encoder, 0));
  const beacon = new PetalDecodedFrame(beaconDecoded.pose, beaconDecoded.rotation, beaconDecoded.mirrored, beaconDecoded.d, NO_FIELDS, null);
  session._absorb(beacon);
  assert.deepEqual(session.progress().meta, encoder.meta);
  assert.equal(session._held[0].evidence[0], evidence);
  assert.equal(session._merge(image, partial), "0123");
  // a timeout forgets the stream and its held evidence
  session.push(new PetalLuma(320, 240), 0);
  session.push(new PetalLuma(320, 240), 60_000);
  assert.equal(session._held.length, 0);
});

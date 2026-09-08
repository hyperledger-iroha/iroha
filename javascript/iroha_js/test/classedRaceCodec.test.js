import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { CLASSED_RACE_VALUE_NAMES_V1, CLASSED_RACE_MAX_VALUE_BYTES_V1,
  encodeClassedRaceValueV1 as encode, decodeClassedRaceValueV1 as decode,
  decodeClassedRaceFrameV1 as decodeFrame } from "../src/classedRace.js";

const fixtureBytes = readFileSync(new URL("fixtures/classed-race-v1-codec.json", import.meta.url));
const fixture = JSON.parse(fixtureBytes);
const provenance = JSON.parse(readFileSync(new URL("fixtures/classed-race-v1-codec.provenance.json", import.meta.url), "utf8"));
assert.equal(createHash("sha256").update(fixtureBytes).digest("hex"), provenance.fixture_sha256);
assert.equal(fixture.version, 1);
assert.equal(fixture.vectors.length, 36);
assert.deepEqual([...new Set(fixture.vectors.map(row => row.name))].sort(), [...CLASSED_RACE_VALUE_NAMES_V1].sort());
const rowOf = (name, label) => fixture.vectors.find(row => row.name === name && (label === undefined || row.case === label));
const valueOf = (name, label) => structuredClone(rowOf(name, label).value);
function normalize(value) {
  if (Array.isArray(value)) return value.map(normalize);
  if (value && typeof value === "object") return Object.fromEntries(Object.entries(value)
    .map(([key, val]) => [key, key === "progress_mm" ? String(val) : normalize(val)]));
  return value;
}
function field(bytes, fieldIndex) {
  let cursor = 0;
  for (let index = 0; ; index++) {
    let length = 0, shift = 0, byte;
    do { byte = bytes[cursor++]; length += (byte & 127) * 2 ** shift; shift += 7; } while (byte & 128);
    if (index === fieldIndex) return bytes.subarray(cursor, cursor + length);
    cursor += length;
  }
}

for (const row of fixture.vectors) test(`native Touring ${row.name}/${row.case} JSON, bare and framed bytes`, () => {
  assert.equal(row.rust_type_name, `iroha_data_model::classed_race_v1::${row.name}`);
  assert.deepEqual(JSON.parse(row.json), row.value);
  const bytes = encode(row.name, row.value), expected = Buffer.from(row.encoded_hex, "hex");
  assert.deepEqual(Buffer.from(bytes), expected);
  assert.deepEqual(decode(row.name, expected), normalize(row.value));
  assert.deepEqual(Buffer.from(encode(row.name, decode(row.name, bytes))), expected);
  const framed = Buffer.from(row.framed_hex, "hex");
  assert.deepEqual(decodeFrame(row.name, framed), normalize(row.value));
  assert.throws(() => decode(row.name, Buffer.concat([expected, Buffer.of(0)])), /trailing|canonical|width|tag|exactly/i);
  assert.throws(() => decode(row.name, framed));
  assert.throws(() => decodeFrame(row.name, expected));
  for (const mutate of [frame => { frame[4] ^= 1; }, frame => { frame[6] ^= 1; },
    frame => { frame[22] = 1; }, frame => { frame[31] ^= 1; }, frame => { frame[39] ^= 2; }]) {
    const altered = Buffer.from(framed); mutate(altered);
    assert.throws(() => decodeFrame(row.name, altered));
  }
  assert.throws(() => decodeFrame(row.name, Buffer.concat([framed, Buffer.of(0)])));
});

test("Touring exact nine-shape dispatch has no stock, instruction or proof fallback", () => {
  for (const name of ["RaceStateV1", "OpenRaceV1", "ClassedRaceProofPayloadV1", "ClassedRaceProverRequestV1", "toString", "__proto__"]) {
    assert.throws(() => encode(name, {}), /unknown/);
    assert.throws(() => decode(name, Buffer.alloc(0)), /unknown/);
    assert.throws(() => decodeFrame(name, Buffer.alloc(40)), /unknown/);
  }
});

test("every Touring shape rejects missing, extra, symbolic and accessor fields", () => {
  for (const name of CLASSED_RACE_VALUE_NAMES_V1) {
    const source = valueOf(name);
    for (const key of Object.keys(source)) {
      const missing = { ...source }; delete missing[key];
      assert.throws(() => encode(name, missing), /field/);
    }
    assert.throws(() => encode(name, { ...source, unexpected: true }), /field/);
    assert.throws(() => encode(name, { ...source, [Symbol("unexpected")]: true }), /field/);
    let accessed = false;
    const accessor = { ...source };
    Object.defineProperty(accessor, Object.keys(source)[0], { enumerable: true, get() { accessed = true; return 0; } });
    assert.throws(() => encode(name, accessor), /data property/);
    assert.equal(accessed, false);
  }
});

test("Touring enums reject unknown tags, alternate names and unit payloads", () => {
  for (const name of ["ClassedRaceClassV1", "ClassedRaceTrackV1"]) {
    for (const value of [{ kind: "stock", value: null }, { kind: "TouringS1", value: null },
      { ...valueOf(name), value: 0 }, { ...valueOf(name), value: {} }]) assert.throws(() => encode(name, value));
    const unknown = Buffer.alloc(4); unknown.writeUInt32LE(name === "ClassedRaceClassV1" ? 1 : 3);
    assert.throws(() => decode(name, unknown), /unknown/);
  }
});

test("Touring frames and replays enforce all input, version and roster bounds", () => {
  const name = "ClassedRaceReplayV1", good = valueOf(name, "harbor_six_tick_technical_win");
  const bad = [value => { value.version = 0; }, value => { value.version = 2; },
    value => { value.player_count = 0; }, value => { value.player_count = 9; },
    value => { value.frames[0].tick = 1; }, value => { value.frames[1].tick = 0; },
    value => { value.frames[0].controls.pop(); }, value => { value.frames[0].controls[0] = 64; },
    value => { value.frames[0].controls[0] = -1; }, value => { value.frames[0].controls[0] = 0.5; },
    value => { value.frames[0].controls = Array(2); }, value => { value.frames = Array(5401); },
    value => { value.dnf_events[0].tick = 7; }, value => { value.dnf_events[0].slots = []; },
    value => { value.dnf_events[0].slots = [1, 0]; }, value => { value.dnf_events[0].slots = [0, 0]; },
    value => { value.dnf_events[0].slots = [2]; },
    value => { value.dnf_events.unshift({ tick: 5, slots: [1] }); },
    value => { value.dnf_events.push({ tick: 6, slots: [0] }); }];
  for (const mutate of bad) { const value = structuredClone(good); mutate(value); assert.throws(() => encode(name, value)); }
  for (const frame of [{ tick: 5400, controls: [0] }, { tick: 0, controls: [] }, { tick: 0, controls: Array(9).fill(0) }]) {
    assert.throws(() => encode("ClassedRaceInputFrameV1", frame));
  }
  const max = { ...good, dnf_events: [], player_count: 8,
    frames: Array.from({ length: 5400 }, (_, tick) => ({ tick, controls: [0, 1, 2, 4, 8, 16, 32, 63] })) };
  assert.equal(decode(name, encode(name, max)).frames.length, 5400);
});

test("Touring state checks exact track, integer and optional timestamp domains", () => {
  const name = "ClassedRaceStateV1", good = valueOf(name, "tied_finish");
  const mutations = [value => { value.tick = 5401; }, value => { value.cars = []; },
    value => { value.cars = Array(9).fill(value.cars[0]); }, value => { value.cars[0].progress_mm = "9".repeat(1000); },
    value => { value.cars[0].progress_mm = -12001; }, value => { value.cars[0].progress_mm = 6_000_001; },
    value => { value.cars[0].lateral_mm = 15301; }, value => { value.cars[0].speed_mm_per_tick = 3301; },
    value => { value.cars[0].lateral_velocity_mm_per_tick = -321; }, value => { value.cars[0].boost_energy = 1001; },
    value => { value.cars[0].finish_tick = null; }, value => { value.cars[0].finish_tick = 0; },
    value => { value.cars[0].finish_tick = 7; }, value => { value.cars[0].dnf_tick = 7; },
    value => { value.cars[0].dnf_tick = 4; }, value => { value.cars[0].boost_energy = -0; }];
  for (const mutate of mutations) { const value = structuredClone(good); mutate(value); assert.throws(() => encode(name, value)); }
});

test("Touring result flags and winner slots distinguish prefixes, ties and refunds", () => {
  const name = "ClassedRaceResultV1";
  const prefix = valueOf(name, "finished_before_batch_boundary");
  assert.equal(prefix.terminal, false); assert.deepEqual(prefix.winners, []);
  assert.throws(() => encode(name, { ...prefix, terminal: true }));
  assert.throws(() => encode(name, { ...prefix, winners: [0] }));
  assert.throws(() => encode(name, { ...prefix, terminal: 0 }));
  const tied = valueOf(name, "tied_finish"); assert.deepEqual(tied.winners, [0, 1]);
  for (const winners of [[1, 0], [0, 0], [2]]) assert.throws(() => encode(name, { ...tied, winners }));
  assert.throws(() => encode(name, { ...tied, standings: [tied.standings[0], tied.standings[0]] }));
  const refund = valueOf(name, "all_forfeit_after_finishing");
  assert.equal(refund.terminal, true); assert.deepEqual(refund.winners, []);
  assert.throws(() => encode(name, { ...refund, winners: [0] }));
  const technical = valueOf(name, "harbor_technical_win");
  assert.throws(() => encode(name, { ...technical, winners: [1] }));
});

test("Touring decoders reject noncanonical lengths, option and Boolean tags before acceptance", () => {
  const name = "ClassedRaceResultV1", good = Buffer.from(rowOf(name, "tied_finish").encoded_hex, "hex");
  const bool = Buffer.from(good); field(bool, 3)[0] = 2;
  assert.throws(() => decode(name, bool), /boolean/);
  const car = Buffer.from(rowOf("ClassedRaceCarStateV1", "none_options").encoded_hex, "hex");
  field(car, 5)[0] = 2;
  assert.throws(() => decode("ClassedRaceCarStateV1", car), /option/);
  const overlong = Buffer.concat([Buffer.of(good[0] | 128, 0), good.subarray(1)]);
  assert.throws(() => decode(name, overlong), /canonical|length/);
  const controls = Buffer.from(rowOf("ClassedRaceInputFrameV1").encoded_hex, "hex");
  field(controls, 1).writeBigUInt64LE(9n);
  assert.throws(() => decode("ClassedRaceInputFrameV1", controls), /limit|bound/);
  assert.throws(() => decode(name, new Uint8Array(CLASSED_RACE_MAX_VALUE_BYTES_V1 + 1)), /bounded/);
  assert.throws(() => decode(name, [0]), /Uint8Array/);
});

import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { deriveRaceResultV1 as derive,
  deriveRaceOutcomeV1, isRaceTerminalStateV1, encodeRaceValueV1, decodeRaceValueV1 } from "../src/race.js";

// JS construction cases are policy tests, not native-origin parity fixtures.
function state(count = 4, tick = 24, kind = "harbor") {
  return { tick, track: { kind, value: null }, cars: Array.from({ length: count }, () => ({ progress_mm: 0,
    lateral_mm: 0, speed_mm_per_tick: 0, lateral_velocity_mm_per_tick: 0, boost_energy: 1000, finish_tick: null, dnf_tick: null })) };
}
function finish(s, slot, tick) {
  s.cars[slot].progress_mm = { harbor: 7_200_000, neon_tokyo: 6_000_000, sakura: 5_400_000 }[s.track.kind];
  s.cars[slot].finish_tick = tick;
}
test("late-forfeit finished racer loses the prize to the sole eligible racer", () => {
  const s = state(); finish(s, 0, 6); s.cars[0].dnf_tick = 12; s.cars[1].dnf_tick = 18; s.cars[2].dnf_tick = 18;
  const before = structuredClone(s);
  const result = derive(s); assert.deepEqual(result.winners, [3]);
  assert.deepEqual(result.standings.map(car => car.slot), [3, 0, 1, 2]);
  assert.equal(result.standings[1].finish_tick, 6, "a disqualifying forfeit never erases finish history");
  assert.deepEqual(s, before);
});
test("all-forfeit refunds even with every possible number of previously finished cars", () => {
  for (let finished = 0; finished <= 4; finished++) {
    const s = state(); for (let slot = 0; slot < finished; slot++) finish(s, slot, 6 + slot);
    s.cars.forEach(car => { car.dnf_tick = 24; });
    assert.deepEqual(derive(s).winners, []);
  }
});
test("eligible exact finish ties exclude an earlier forfeited finisher and retain every co-winner", () => {
  const s = state(); finish(s, 0, 6); finish(s, 1, 12); finish(s, 2, 12); s.cars[0].dnf_tick = 18;
  assert.deepEqual(derive(s).winners, [1, 2]);
  assert.deepEqual(derive(s).standings.map(car => car.slot), [1, 2, 3, 0]);
  const outcome = deriveRaceOutcomeV1(s);
  assert.deepEqual(outcome.winner_slots, [1, 2]); assert.equal(outcome.terminal_tick, s.tick);
  assert.deepEqual(encodeRaceValueV1("RaceResultV1", derive(s)), Buffer.from(outcome.result));
  assert.deepEqual(decodeRaceValueV1("RaceResultV1", Buffer.from(outcome.result)).winners, [1, 2]);
});
test("timeout distance compares only eligible racers; one millimetre breaks distance but slot cannot break ties", () => {
  const s = state(4, 5400); finish(s, 0, 6); s.cars[0].dnf_tick = 12;
  s.cars[1].progress_mm = 400_000; s.cars[2].progress_mm = 400_000; s.cars[3].progress_mm = 399_999;
  assert.deepEqual(derive(s).winners, [1, 2]);
  s.cars[2].progress_mm++; assert.deepEqual(derive(s).winners, [2]);
  assert.deepEqual(derive(s).standings.map(car => car.slot), [2, 1, 3, 0]);
});
test("unexceptional distance outcomes preserve all eight eligible winners on each track", () => {
  for (const kind of ["neon_tokyo", "harbor", "sakura"]) {
    const s = state(8, 5400, kind), result = derive(s);
    assert.deepEqual(result.winners, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert.deepEqual(deriveRaceOutcomeV1(s).winner_slots, result.winners);
  }
});
test("batch boundaries, one-car practice and unfinished nonterminal states match frozen native rules", () => {
  const solo = state(1, 0); assert.equal(isRaceTerminalStateV1(solo), false); assert.deepEqual(derive(solo).winners, []);
  solo.tick = 5400; assert.equal(isRaceTerminalStateV1(solo), true); assert.deepEqual(derive(solo).winners, []);
  const s = state(2, 23); s.cars[0].dnf_tick = 18;
  assert.equal(isRaceTerminalStateV1(s), false); assert.deepEqual(derive(s).winners, []);
  s.tick = 24; assert.equal(isRaceTerminalStateV1(s), true); assert.deepEqual(derive(s).winners, [1]);
  const unfinished = state(); assert.equal(isRaceTerminalStateV1(unfinished), false); assert.deepEqual(derive(unfinished).winners, []);
  // Native result derivation may describe a nonterminal prefix; proving admission is separate.
  finish(unfinished, 0, 6); assert.equal(isRaceTerminalStateV1(unfinished), false); assert.deepEqual(derive(unfinished).winners, [0]);
});
test("native state bounds reject impossible numbers, overflow, malformed shapes and inconsistent finish history", () => {
  const changes = [s => { s.tick = 5401; }, s => { s.cars = []; }, s => { s.cars = Array(9).fill(s.cars[0]); },
    s => { s.cars[0].progress_mm = -12001; }, s => { s.cars[0].progress_mm = 7_200_001; },
    s => { s.cars[0].lateral_mm = 16201; }, s => { s.cars[0].lateral_mm = -16201; },
    s => { s.cars[0].speed_mm_per_tick = 3001; }, s => { s.cars[0].speed_mm_per_tick = -1; },
    s => { s.cars[0].lateral_velocity_mm_per_tick = 321; }, s => { s.cars[0].boost_energy = 1001; },
    s => { s.cars[0].finish_tick = 0; }, s => { finish(s, 0, 25); }, s => { s.cars[0].finish_tick = 6; },
    s => { s.cars[0].dnf_tick = 25; }, s => { s.cars[0].progress_mm = 1.5; },
    s => { s.cars[0].progress_mm = "1".repeat(1000); }, s => { s.cars[0].extra = "award"; },
    s => { s.cars[0].finish_tick = undefined; }, s => { s.track.value = 1; }];
  for (const mutation of changes) { const s = state(); mutation(s); assert.throws(() => derive(s)); }
  const boundary = state(); Object.assign(boundary.cars[0], { progress_mm: -12000, lateral_mm: 16200, speed_mm_per_tick: 3000, lateral_velocity_mm_per_tick: -320, boost_energy: 0, dnf_tick: 0 });
  assert.equal(derive(boundary).standings.length, 4);
});
test("state preparation rejects accessors and sparse input without reading hidden mutable values", () => {
  let reads = 0; const s = state(); Object.defineProperty(s.cars[0], "dnf_tick", { enumerable: true, get() { reads++; return 0; } });
  assert.throws(() => derive(s)); assert.equal(reads, 0);
  const sparse = state(); delete sparse.cars[1]; assert.throws(() => derive(sparse));
  const tagged = state(); tagged[Symbol("outcome")] = "winner"; assert.throws(() => derive(tagged));
});

test("native-origin outcome corpus matches every standing, winner and canonical outcome byte", () => {
  const corpus = JSON.parse(readFileSync(new URL("fixtures/race-outcomes-v1.json", import.meta.url), "utf8"));
  assert.equal(corpus.version, 1); assert.equal(corpus.valid.length, 9); assert.equal(corpus.invalid.length, 5);
  for (const row of corpus.valid) {
    assert.deepEqual(derive(row.state), row.result, `${row.name}: exact corrected native result`);
    assert.deepEqual(deriveRaceOutcomeV1(row.state), { terminal_tick: row.result.ticks, winner_slots: row.result.winners,
      result: Array.from(encodeRaceValueV1("RaceResultV1", row.result)) });
  }
  for (const row of corpus.invalid) assert.throws(() => derive(row.state), undefined, row.name);
});

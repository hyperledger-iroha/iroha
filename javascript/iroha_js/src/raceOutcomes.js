/**
 * Canonical RaceV1 award policy for the first Iroha game release.
 * This validates native state bounds, not reachability, terminal admission,
 * a proof, or authenticated ledger state.
 */
import { noritoEncodeGameValueV1, noritoDecodeGameValueV1 } from "./norito.js";

const MAX_TICKS = 5400;
const FINISH = Object.freeze({ neon_tokyo: 6_000_000, harbor: 7_200_000, sakura: 5_400_000 });
const CAR_FIELDS = ["progress_mm", "lateral_mm", "speed_mm_per_tick", "lateral_velocity_mm_per_tick", "boost_energy", "finish_tick", "dnf_tick"];
function exact(value, keys, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)
      || ![Object.prototype, null].includes(Object.getPrototypeOf(value))
      || Reflect.ownKeys(value).length !== keys.length) throw new TypeError(`${label} requires exact native data fields`);
  for (const key of keys) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor?.enumerable || !Object.hasOwn(descriptor, "value")) throw new TypeError(`${label} requires exact native data fields`);
  }
}
function numericInput(value) {
  if (value === undefined) throw new TypeError("race state requires explicit numeric values or optional nulls");
  if (typeof value === "string" && value.length > 20) throw new RangeError("race state integer exceeds native width");
}
function stateValue(input) {
  exact(input, ["tick", "track", "cars"], "race state");
  exact(input.track, ["kind", "value"], "race track");
  if (!Array.isArray(input.cars) || Object.getPrototypeOf(input.cars) !== Array.prototype
      || input.cars.length < 1 || input.cars.length > 8 || Reflect.ownKeys(input.cars).length !== input.cars.length + 1) {
    throw new RangeError("race state requires 1–8 dense car records");
  }
  numericInput(input.tick);
  for (let slot = 0; slot < input.cars.length; slot++) {
    const descriptor = Object.getOwnPropertyDescriptor(input.cars, String(slot));
    if (!descriptor?.enumerable || !Object.hasOwn(descriptor, "value")) throw new TypeError("race cars require dense data elements");
    exact(descriptor.value, CAR_FIELDS, "race car");
    for (const field of CAR_FIELDS) numericInput(descriptor.value[field]);
  }
  const state = noritoDecodeGameValueV1("RaceStateV1", noritoEncodeGameValueV1("RaceStateV1", input));
  const finish = FINISH[state.track.kind];
  if (state.tick > MAX_TICKS || state.cars.some(car => {
    const progress = BigInt(car.progress_mm);
    return progress < -12_000n || progress > BigInt(finish)
      || car.lateral_mm < -16_200 || car.lateral_mm > 16_200
      || car.speed_mm_per_tick < 0 || car.speed_mm_per_tick > 3000
      || car.lateral_velocity_mm_per_tick < -320 || car.lateral_velocity_mm_per_tick > 320
      || car.boost_energy > 1000
      || car.finish_tick !== null && (car.finish_tick === 0 || car.finish_tick > state.tick || progress !== BigInt(finish))
      || car.dnf_tick !== null && car.dnf_tick > state.tick;
  })) throw new RangeError("race state violates frozen native bounds");
  return state;
}
function standings(state) {
  return state.cars.map((car, slot) => ({ slot, finish_tick: car.finish_tick,
    dnf_tick: car.dnf_tick, progress_mm: Number(car.progress_mm) }));
}
function finishOrder(a, b) {
  if (a.finish_tick !== null && b.finish_tick !== null) return a.finish_tick - b.finish_tick;
  if (a.finish_tick !== null) return -1;
  if (b.finish_tick !== null) return 1;
  return 0;
}
function finishOrDistance(a, b) {
  return finishOrder(a, b) || (a.finish_tick === null && b.finish_tick === null ? b.progress_mm - a.progress_mm : 0);
}
function fallbackWinners(state, eligible) {
  if (state.cars.length < 2 || state.tick % 6 !== 0 || eligible.length === 0) return [];
  if (eligible.length === 1) return [eligible[0].slot];
  if (state.tick === MAX_TICKS) return eligible.filter(car => car.progress_mm === eligible[0].progress_mm).map(car => car.slot);
  return [];
}
/** Forfeited racers rank below every eligible racer, preserving exact eligible ties. */
export function deriveRaceResultV1(input) {
  const state = stateValue(input), ranked = standings(state);
  ranked.sort((a, b) => Number(a.dnf_tick !== null) - Number(b.dnf_tick !== null)
    || finishOrDistance(a, b) || a.slot - b.slot);
  const eligible = ranked.filter(car => car.dnf_tick === null);
  const earliest = eligible[0]?.finish_tick;
  const winners = earliest !== null && earliest !== undefined
    ? eligible.filter(car => car.finish_tick === earliest).map(car => car.slot)
    : fallbackWinners(state, eligible);
  return { ticks: state.tick, standings: ranked, winners };
}
function outcome(result) {
  return { terminal_tick: result.ticks, winner_slots: [...result.winners],
    result: Array.from(noritoEncodeGameValueV1("RaceResultV1", result)) };
}
/** Canonical generic outcome; does not prove terminal admission. */
export function deriveRaceOutcomeV1(state) { return outcome(deriveRaceResultV1(state)); }
/** Frozen stock terminal boundaries, including the one-car practice exception. */
export function isRaceTerminalStateV1(input) {
  const state = stateValue(input);
  return state.tick === MAX_TICKS || state.tick % 6 === 0
    && (state.cars.every(car => car.finish_tick !== null || car.dnf_tick !== null)
      || state.cars.length >= 2 && state.cars.filter(car => car.dnf_tick === null).length < 2);
}

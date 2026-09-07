/** Closed application-owned Touring model schemas; no proof or ledger admission adapter. */
export const CLASSED_RACE_MAX_TICKS_V1 = 5400;
export const CLASSED_RACE_MAX_PLAYERS_V1 = 8;
export const CLASSED_RACE_MAX_VALUE_BYTES_V1 = 1024 * 1024;
export const CLASSED_RACE_CLASSES_V1 = Object.freeze(["touring_s1"]);
export const CLASSED_RACE_TRACKS_V1 = Object.freeze(["neon_tokyo", "harbor", "sakura"]);
const fields = text => text.split(" ").map(entry => entry.split(":"));
export const CLASSED_RACE_SCHEMAS_V1 = Object.freeze({
  ClassedRaceInputFrameV1: fields("tick:u32 controls:frameControls"),
  ClassedRaceDnfEventV1: fields("tick:u32 slots:slots"),
  ClassedRaceReplayV1: fields("version:u16 class_id:ClassedRaceClassV1 track:ClassedRaceTrackV1 player_count:u8 frames:classedFrames dnf_events:classedDnfEvents"),
  ClassedRaceCarStateV1: fields("progress_mm:i64 lateral_mm:i32 speed_mm_per_tick:i32 lateral_velocity_mm_per_tick:i32 boost_energy:u16 finish_tick:optionalU32 dnf_tick:optionalU32"),
  ClassedRaceStateV1: fields("tick:u32 class_id:ClassedRaceClassV1 track:ClassedRaceTrackV1 cars:classedCars"),
  ClassedRaceStandingV1: fields("slot:u8 finish_tick:optionalU32 dnf_tick:optionalU32 progress_mm:i64"),
  ClassedRaceResultV1: fields("class_id:ClassedRaceClassV1 track:ClassedRaceTrackV1 ticks:u32 terminal:bool standings:classedStandings winners:slots"),
});
export const CLASSED_RACE_VECTORS_V1 = Object.freeze({
  classedFrames: ["ClassedRaceInputFrameV1", CLASSED_RACE_MAX_TICKS_V1],
  classedDnfEvents: ["ClassedRaceDnfEventV1", CLASSED_RACE_MAX_PLAYERS_V1],
  classedCars: ["ClassedRaceCarStateV1", CLASSED_RACE_MAX_PLAYERS_V1],
  classedStandings: ["ClassedRaceStandingV1", CLASSED_RACE_MAX_PLAYERS_V1],
});
export const CLASSED_RACE_VALUE_NAMES_V1 = Object.freeze([
  "ClassedRaceClassV1", "ClassedRaceTrackV1", ...Object.keys(CLASSED_RACE_SCHEMAS_V1),
]);
const FINISH = Object.freeze({ neon_tokyo: 6_000_000, harbor: 7_200_000, sakura: 5_400_000 });

// This checks application bounds before the shared primitive codec parses i64 values.
// Wire integer, length, option, vector and frame encoding remains in the shared Norito codecs.
function bounded(value, low, high, label) {
  if (!(typeof value === "number" && Number.isSafeInteger(value) && !Object.is(value, -0))
      && !(typeof value === "string" && value.length <= 20 && /^(?:0|-?[1-9][0-9]*)$/.test(value))
      && typeof value !== "bigint") throw new TypeError(`${label} requires a canonical integer`);
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number < low || number > high) {
    throw new RangeError(`${label} must be ${low}..${high}`);
  }
  return number;
}
function timestamps(value, tick = CLASSED_RACE_MAX_TICKS_V1) {
  if (value.finish_tick !== null) bounded(value.finish_tick, 1, tick, "classed finish tick");
  if (value.dnf_tick !== null) bounded(value.dnf_tick, 0, tick, "classed removal tick");
  if (value.finish_tick !== null && value.dnf_tick !== null
      && Number(value.finish_tick) > Number(value.dnf_tick)) {
    throw new RangeError("classed finish cannot occur after removal");
  }
}

/** Scalar preflight after exact own-field checks, before any encoding or nested traversal. */
export function validateClassedRaceScalarsV1(name, value) {
  if (!Object.hasOwn(CLASSED_RACE_SCHEMAS_V1, name)) return;
  if (Object.hasOwn(value, "tick")) bounded(value.tick, 0,
    name === "ClassedRaceInputFrameV1" ? CLASSED_RACE_MAX_TICKS_V1 - 1 : CLASSED_RACE_MAX_TICKS_V1,
    `${name}.tick`);
  if (name === "ClassedRaceReplayV1") {
    bounded(value.version, 1, 1, "classed replay version");
    bounded(value.player_count, 1, CLASSED_RACE_MAX_PLAYERS_V1, "classed player count");
  }
  if (Object.hasOwn(value, "progress_mm")) {
    bounded(value.progress_mm, -12_000, 7_200_000, `${name}.progress_mm`);
    timestamps(value);
  }
  if (name === "ClassedRaceCarStateV1") {
    bounded(value.lateral_mm, -15_300, 15_300, "classed lateral position");
    bounded(value.speed_mm_per_tick, 0, 3300, "classed speed");
    bounded(value.lateral_velocity_mm_per_tick, -320, 320, "classed lateral velocity");
    bounded(value.boost_energy, 0, 1000, "classed boost energy");
  }
  if (name === "ClassedRaceStandingV1") bounded(value.slot, 0, 7, "classed standing slot");
  if (name === "ClassedRaceResultV1") bounded(value.ticks, 0, CLASSED_RACE_MAX_TICKS_V1, "classed result ticks");
}
function orderedSlots(slots, maximum, label, nonempty = false) {
  if (nonempty && slots.length === 0) throw new RangeError(`${label} cannot be empty`);
  let previous = -1;
  for (const value of slots) {
    const slot = Number(value);
    if (slot <= previous || slot >= maximum) throw new RangeError(`${label} requires ascending unique existing slots`);
    previous = slot;
  }
}
function stateRows(rows, track, tick) {
  if (rows.length < 1) throw new RangeError("classed state or standings requires one to eight cars");
  const finish = FINISH[track.kind];
  for (const car of rows) {
    const progress = bounded(car.progress_mm, -12_000, finish, "classed track progress");
    timestamps(car, tick);
    if ((car.finish_tick !== null) !== (progress === finish)) {
      throw new RangeError("classed finish tick and track progress disagree");
    }
  }
}

/** Validate bounded model structure; this does not prove reachability, ranking or NFT admission. */
export function validateClassedRaceRecordV1(name, value) {
  if (!Object.hasOwn(CLASSED_RACE_SCHEMAS_V1, name)) return;
  validateClassedRaceScalarsV1(name, value);
  if (name === "ClassedRaceInputFrameV1" && value.controls.length === 0) {
    throw new RangeError("classed input frame requires one to eight controls");
  }
  if (name === "ClassedRaceDnfEventV1") orderedSlots(value.slots, 8, "classed removals", true);
  if (name === "ClassedRaceReplayV1") {
    const players = Number(value.player_count);
    for (const [tick, frame] of value.frames.entries()) {
      if (Number(frame.tick) !== tick || frame.controls.length !== players) {
        throw new RangeError("classed replay frames must be consecutive and match its roster");
      }
    }
    const removed = new Set();
    let previous = -1;
    for (const event of value.dnf_events) {
      const tick = Number(event.tick);
      if (tick <= previous || tick > value.frames.length) throw new RangeError("classed removal ticks must be ordered inside the replay");
      orderedSlots(event.slots, players, "classed replay removals", true);
      for (const slot of event.slots.map(Number)) {
        if (removed.has(slot)) throw new RangeError("classed input key cannot be removed twice");
        removed.add(slot);
      }
      previous = tick;
    }
  }
  if (name === "ClassedRaceStateV1") stateRows(value.cars, value.track, Number(value.tick));
  if (name === "ClassedRaceResultV1") {
    const ticks = Number(value.ticks), rows = value.standings;
    stateRows(rows, value.track, ticks);
    const slots = new Set(rows.map(row => Number(row.slot)));
    if (slots.size !== rows.length || [...slots].some(slot => slot >= rows.length)) {
      throw new RangeError("classed standings must contain each original slot exactly once");
    }
    orderedSlots(value.winners, rows.length, "classed winners");
    const eligible = rows.filter(row => row.dnf_tick === null);
    const terminal = ticks === CLASSED_RACE_MAX_TICKS_V1 || ticks % 6 === 0
      && (rows.every(row => row.finish_tick !== null || row.dnf_tick !== null)
        || rows.length >= 2 && eligible.length < 2);
    if (value.terminal !== terminal) throw new RangeError("classed terminal flag disagrees with its tick and standings");
    if (!terminal && value.winners.length > 0 || eligible.length === 0 && value.winners.length > 0
        || value.winners.some(slot => !eligible.some(row => Number(row.slot) === Number(slot)))) {
      throw new RangeError("classed prefix/refund cannot have winners and removed cars cannot win");
    }
  }
}

import { noritoEncodeGameValueV1, noritoDecodeGameValueV1 } from "./norito.js";
import { gameMessageHashV1, gameHashLiteralV1 } from "./game.js";
export { deriveRaceResultV1, deriveRaceOutcomeV1, isRaceTerminalStateV1 } from "./raceOutcomes.js";
const TYPES = new Set(["RaceProverRequestV1", "RaceProofPayloadV1", "RaceTrackV1", "RaceRulesV1", "RaceCheckpointV1", "RaceSlotSignatureV1", "SignedRaceCheckpointV1", "RaceCommitmentSetBodyV1", "RaceCommitmentSetV1", "RaceInputCommitmentBodyV1", "RaceInputCommitmentV1", "RaceInputRevealV1", "RaceChallengeBodyV1", "RaceInputFrameV1", "RaceDnfEventV1", "RaceReplayV1", "RaceCarStateV1", "RaceStateV1", "RaceStandingV1", "RaceResultV1", "RacePublicInputsV1"]);
/** Compiled SORA CARS adapter codecs. Native chain instructions are generic game-session instructions. */
export function encodeRaceValueV1(name, value) { if (!TYPES.has(name)) throw new TypeError("unknown SORA CARS adapter type"); return noritoEncodeGameValueV1(name, value); }
export function decodeRaceValueV1(name, bytes) { if (!TYPES.has(name)) throw new TypeError("unknown SORA CARS adapter type"); return noritoDecodeGameValueV1(name, bytes); }
export const raceHashLiteralV1 = gameHashLiteralV1;
export function raceInputPayloadV1(controls) {
  if (!Array.isArray(controls) || controls.length !== 6 || controls.some(word => !Number.isInteger(word) || word < 0 || word > 63)) throw new TypeError("SORA CARS input requires six control words");
  return controls.flatMap(word => [word & 255, word >>> 8]);
}
export function raceControlsFromPayloadV1(payload) {
  if (!Array.isArray(payload) || payload.length !== 12 || payload.some(byte => !Number.isInteger(byte) || byte < 0 || byte > 255)) throw new TypeError("SORA CARS payload requires twelve bytes");
  const controls = Array.from({ length: 6 }, (_, i) => payload[i * 2] | payload[i * 2 + 1] << 8);
  raceInputPayloadV1(controls); return controls;
}
export function raceGameTranscriptV1(replay) {
  encodeRaceValueV1("RaceReplayV1", replay);
  if (replay.frames.length % 6) throw new TypeError("SORA CARS transcripts require complete six-tick batches");
  const batches = [];
  for (let tick = 0; tick < replay.frames.length; tick += 6) batches.push({ start_tick: tick, inputs: Array.from({ length: replay.player_count }, (_, slot) => raceInputPayloadV1(replay.frames.slice(tick, tick + 6).map(frame => frame.controls[slot]))) });
  return { batches, dnf_events: structuredClone(replay.dnf_events) };
}
export function raceGameplayHashV1(network, domain, value) {
  if (domain === "input-transcript") return gameMessageHashV1(network, domain, raceGameTranscriptV1(value));
  if (domain === "simulation-state") return gameMessageHashV1(network, domain, Array.from(encodeRaceValueV1("RaceStateV1", value)));
  const { race_id, ...body } = value;
  const generic = { session_id: race_id, ...body };
  if (domain === "input-reveal") { generic.payload = raceInputPayloadV1(generic.controls); delete generic.controls; }
  return gameMessageHashV1(network, domain, generic);
}

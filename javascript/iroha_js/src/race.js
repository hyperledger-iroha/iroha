import { Buffer } from "buffer";
import { blake2b256 } from "./blake2b.js";
import { NetworkId, networkIdBytes } from "./networkId.js";
import { computeHashLiteralCrc } from "./hashLiteralCrc.js";
import { noritoEncodeRaceValueV1, noritoDecodeRaceValueV1 } from "./norito.js";
import { RACE_INSTRUCTION_NAMES_V1, RACE_INSTRUCTION_WIRE_IDS_V1 } from "./noritoRaceCodecs.js";

export { RACE_INSTRUCTION_NAMES_V1, RACE_INSTRUCTION_WIRE_IDS_V1 };
export const encodeRaceValueV1 = noritoEncodeRaceValueV1;
export const decodeRaceValueV1 = noritoDecodeRaceValueV1;
const DOMAINS = Object.freeze({
  "input-reveal": "RaceInputRevealV1",
  "input-commitment": "RaceInputCommitmentBodyV1",
  "commitment-set": "RaceCommitmentSetBodyV1",
  checkpoint: "RaceCheckpointV1",
  challenge: "RaceChallengeBodyV1",
  "input-transcript": "RaceReplayV1",
  "simulation-state": "RaceStateV1",
});

/** Render exact marked consensus hash bytes as a checksummed hash literal. */
export function raceHashLiteralV1(bytes) {
  const value = Buffer.from(bytes);
  if (value.length !== 32 || (value[31] & 1) === 0) throw new TypeError("race hash must contain 32 marked bytes");
  const body = value.toString("hex").toUpperCase();
  return `hash:${body}#${computeHashLiteralCrc("hash", body)}`;
}

/** Match data_model::race::race_message_hash_v1 exactly; gameplay keys never sign transactions. */
export function raceGameplayHashV1(networkId, domain, value) {
  if (!Object.hasOwn(DOMAINS, domain)) throw new TypeError("unknown native race gameplay domain");
  const network = typeof networkId === "string" ? NetworkId.parse(networkId) : networkId;
  const payload = Buffer.concat([
    Buffer.from("iroha:race:gameplay:v1\0", "utf8"),
    Buffer.from(networkIdBytes(network, "race gameplay network")),
    Buffer.from(domain, "utf8"),
    noritoEncodeRaceValueV1(DOMAINS[domain], value),
  ]);
  const hash = Uint8Array.from(blake2b256(payload));
  hash[31] |= 1;
  return hash;
}

/** Validate and normalize a single closed native race instruction before wallet construction. */
export function buildRaceInstructionV1(name, value) {
  if (!RACE_INSTRUCTION_NAMES_V1.includes(name)) throw new TypeError("unknown native race instruction");
  return { [name]: noritoDecodeRaceValueV1(name, noritoEncodeRaceValueV1(name, value)) };
}

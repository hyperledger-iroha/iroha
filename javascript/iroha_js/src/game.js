import { Buffer } from "buffer";
import { blake2b256 } from "./blake2b.js";
import { NetworkId, networkIdBytes } from "./networkId.js";
import { computeHashLiteralCrc } from "./hashLiteralCrc.js";
import { noritoEncodeGameValueV1, noritoDecodeGameValueV1 } from "./norito.js";
import { GAME_INSTRUCTION_NAMES_V1, GAME_INSTRUCTION_WIRE_IDS_V1, EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1 } from "./noritoGameCodecs.js";
export { GAME_INSTRUCTION_NAMES_V1, GAME_INSTRUCTION_WIRE_IDS_V1, EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1 };
export * from "./gameResources.js";
export { GAME_ADMISSION_MAX_ACCOUNT_ENCODED_BYTES_V1, GAME_ADMISSION_MAX_NFT_ENCODED_BYTES_V1, GAME_ADMISSION_MAX_BYTES_V1 } from "./noritoGameCodecs.js";
export const encodeGameValueV1 = noritoEncodeGameValueV1;
export const decodeGameValueV1 = noritoDecodeGameValueV1;
const DOMAINS = Object.freeze({
  roster: "GameAdmissionCommitmentV1",
  "input-reveal": "GameInputRevealV1", "input-commitment": "GameInputCommitmentBodyV1",
  "commitment-set": "GameCommitmentSetBodyV1", checkpoint: "GameCheckpointV1",
  challenge: "GameChallengeBodyV1", invitation: "GameInvitationBodyV1",
  "input-transcript": "GameTranscriptV1", "simulation-state": "OpaqueBytes",
  "session-manifest": "GameManifestV1", "session-outcome": "GameOutcomeV1",
});
export function gameHashLiteralV1(bytes) {
  const value = Buffer.from(bytes);
  if (value.length !== 32 || (value[31] & 1) === 0) throw new TypeError("game hash must contain 32 marked bytes");
  const body = value.toString("hex").toUpperCase();
  return `hash:${body}#${computeHashLiteralCrc("hash", body)}`;
}
/** Exact domain-separated opaque game-session messages; these signatures cannot spend wallet funds. */
export function gameMessageHashV1(networkId, domain, value) {
  if (!Object.hasOwn(DOMAINS, domain)) throw new TypeError("unknown native game message domain");
  const network = typeof networkId === "string" ? NetworkId.parse(networkId) : networkId;
  const payload = Buffer.concat([Buffer.from("iroha:game:session:v1\0", "utf8"), Buffer.from(networkIdBytes(network, "game session network")), Buffer.from(domain, "utf8"), noritoEncodeGameValueV1(DOMAINS[domain], value)]);
  const hash = Uint8Array.from(blake2b256(payload)); hash[31] |= 1; return hash;
}
/** Validate and normalize a closed generic native game instruction before wallet signing. */
export function buildGameInstructionV1(name, value) {
  if (!GAME_INSTRUCTION_NAMES_V1.includes(name)) throw new TypeError("unknown native game instruction");
  return { [name]: noritoDecodeGameValueV1(name, noritoEncodeGameValueV1(name, value)) };
}

/** Require the exact wallet-approved asset, stake and immutable manifest when joining. */
export function buildJoinGameSessionV1(value) {
  return buildGameInstructionV1("JoinGameSessionV1", value);
}

/** Require the complete canonical original admission body; no mutable-field normalization. */
export function gameRosterHashV1(networkId, sessionId, admission) {
  return gameHashLiteralV1(gameMessageHashV1(networkId, "roster", { session_id: sessionId, admission }));
}
/** Validate and copy the exact public admission body without sorting or defaulting fields. */
export function validateGameAdmissionBodyV1(value) {
  return decodeGameValueV1("GameAdmissionBodyV1", encodeGameValueV1("GameAdmissionBodyV1", value));
}

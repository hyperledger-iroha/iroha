/** Application-owned Touring model codecs. These functions do not install a proof or authorize items. */
import { noritoEncodeGameValueV1, noritoDecodeGameValueV1, validateNoritoFrame } from "./norito.js";
import { CLASSED_RACE_VALUE_NAMES_V1, CLASSED_RACE_MAX_VALUE_BYTES_V1 } from "./noritoClassedRaceSchemas.js";
export { CLASSED_RACE_VALUE_NAMES_V1, CLASSED_RACE_MAX_VALUE_BYTES_V1,
  CLASSED_RACE_MAX_TICKS_V1, CLASSED_RACE_MAX_PLAYERS_V1 } from "./noritoClassedRaceSchemas.js";
const names = new Set(CLASSED_RACE_VALUE_NAMES_V1);
function nameOf(name) {
  if (!names.has(name)) throw new TypeError("unknown application-owned ClassedRaceV1 type");
  return name;
}
function boundedBytes(bytes, maximum) {
  if (!(bytes instanceof Uint8Array) || bytes.byteLength > maximum) {
    throw new TypeError("classed race requires bounded Uint8Array bytes");
  }
  return bytes;
}
/** Encode one of the nine exact native Touring shapes using the canonical bare layout. */
export function encodeClassedRaceValueV1(name, value) {
  return noritoEncodeGameValueV1(nameOf(name), value);
}
/** Decode canonical bare bytes; reject extra fields, unknown tags, suffixes and invalid bounds. */
export function decodeClassedRaceValueV1(name, bytes) {
  return noritoDecodeGameValueV1(nameOf(name), boundedBytes(bytes, CLASSED_RACE_MAX_VALUE_BYTES_V1));
}
/** Validate native type binding, exact flags, checksum and length before decoding a framed value. */
export function decodeClassedRaceFrameV1(name, bytes) {
  nameOf(name);
  const frame = validateNoritoFrame(boundedBytes(bytes, CLASSED_RACE_MAX_VALUE_BYTES_V1 + 40), {
    context: name,
    expectedTypeName: `iroha_data_model::classed_race_v1::${name}`,
    expectedPaddingLength: 0,
    requireNonEmptyPayload: true,
  });
  const flags = name === "ClassedRaceClassV1" || name === "ClassedRaceTrackV1" ? 0 : 2;
  if (frame.flags !== flags) throw new TypeError("classed race frame requires its exact native layout flags");
  return decodeClassedRaceValueV1(name, frame.payload);
}

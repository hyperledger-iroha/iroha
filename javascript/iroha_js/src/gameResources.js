/**
 * Pure preparation helpers for explicit generic game equipment admission.
 * These helpers validate exact authorization; native Core authenticates ownership and custody.
 */
import { Buffer } from "buffer";
import { encodeGameResourceValueV1, decodeGameResourceValueV1,
  GAME_MAX_RESOURCE_PARTICIPANTS_V1, GAME_RESOURCE_MAX_ACCOUNT_ID_BYTES_V1 } from "./noritoGameResourceCodecs.js";
import { noritoEncodeNftMarketValueV1, noritoDecodeNftMarketValueV1 } from "./norito.js";
export * from "./noritoGameResourceCodecs.js";

function canonical(name, value) {
  return decodeGameResourceValueV1(name, encodeGameResourceValueV1(name, value));
}
/** Validate and copy exact, already ordered wallet authorizations; never sort. */
export function validateGameResourceClausesV1(value) { return canonical("clauses", value); }
/** Validate and copy a compiled adapter's requirements; these do not authorize custody. */
export function validateGameResourceRequirementsV1(value) { return canonical("requirements", value); }
/** Require exact explicit authorization, rejecting missing, extra or changed terms. */
export function matchGameResourceRequirementsV1(clauses, requirements) {
  const authorized = encodeGameResourceValueV1("clauses", clauses);
  const required = encodeGameResourceValueV1("requirements", requirements);
  if (!Buffer.from(authorized).equals(required)) throw new TypeError("typed resource clauses do not match compiled requirements");
}
/**
 * Validate retained geometry, optionally checking the original permanent roster.
 * This does not authenticate phase, owner/metadata, non-signable custody derivation
 * or inverse indexes; those remain native consensus admission/restore obligations.
 */
function validateOwners(records, participantOwners) {
  if (participantOwners !== undefined) {
    if (!Array.isArray(participantOwners) || Object.getPrototypeOf(participantOwners) !== Array.prototype
        || participantOwners.length > GAME_MAX_RESOURCE_PARTICIPANTS_V1
        || Reflect.ownKeys(participantOwners).length !== participantOwners.length + 1) {
      throw new TypeError("resource records do not match retained original owners");
    }
    const seen = new Set();
    for (let index = 0; index < participantOwners.length; index++) {
      const descriptor = Object.getOwnPropertyDescriptor(participantOwners, String(index));
      if (!descriptor?.enumerable || !Object.hasOwn(descriptor, "value")) throw new TypeError("roster requires dense data elements");
      const owner = descriptor.value;
      if (typeof owner !== "string" || owner.length > GAME_RESOURCE_MAX_ACCOUNT_ID_BYTES_V1
          || Buffer.byteLength(owner, "utf8") > GAME_RESOURCE_MAX_ACCOUNT_ID_BYTES_V1 || seen.has(owner)
          || noritoDecodeNftMarketValueV1("account", noritoEncodeNftMarketValueV1("account", owner)) !== owner) {
        throw new TypeError("resource roster requires unique canonical accounts");
      }
      seen.add(owner);
    }
    if (records.some(record => participantOwners[record.slot] !== record.original_owner || seen.has(record.custody))) {
      throw new TypeError("resource records do not match retained original owners");
    }
  }
}
export function validateGameResourceReservationSetV1(value, participantOwners) {
  const set = canonical("GameResourceReservationSetV1", value);
  validateOwners(set.records, participantOwners);
  return set;
}
/** Validate retained records embedded in one authoritative native session. */
export function validateGameResourceRecordsV1(value, participantOwners) {
  const records = canonical("records", value);
  validateOwners(records, participantOwners);
  return records;
}

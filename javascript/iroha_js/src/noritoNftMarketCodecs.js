import { Buffer } from "buffer";

const TEXT_PAYMENT_ASSET_ASSET_PRICE_QUANTITY_EXPIRES_AT_HEIGHT_U64_RESERVED_BUYER = " payment_asset:asset price:quantity expires_at_height:u64 reserved_buyer:";
const TEXT_OPTIONAL_ACCOUNT = "optionalAccount";
const TEXT_NATIVE_NFT_CUSTODY_PURPOSE = " native NFT custody purpose";


export const NFT_MARKET_INSTRUCTION_NAMES_V1 = Object.freeze(["OfferNftV1", "BuyNftV1", "CancelNftOfferV1"]);
export const NFT_MARKET_INSTRUCTION_WIRE_IDS_V1 = Object.freeze(NFT_MARKET_INSTRUCTION_NAMES_V1.map(name => `iroha.instruction.v1::nft_market::${name}`));
const fields = value => value.split(" ").map(entry => entry.split(":"));
const schemas = Object.freeze({
  NftCustodyRecordV1: fields(("version:u16 network_id:hash reservation_id:hash purpose:NftCustodyPurposeV1 nft_id:nft custody:account original_owner:account metadata_hash:hash released_to:" + TEXT_OPTIONAL_ACCOUNT)),
  OfferNftV1: fields(("offer_id:hash nft_id:nft" + TEXT_PAYMENT_ASSET_ASSET_PRICE_QUANTITY_EXPIRES_AT_HEIGHT_U64_RESERVED_BUYER + "optionalAccount")),
  BuyNftV1: fields("offer:NftSaleOfferV1"),
  CancelNftOfferV1: fields("offer_id:hash expected_offer_hash:hash"),
  NftSaleOfferV1: fields(("network_id:hash offer_id:hash nft_id:nft seller:account" + TEXT_PAYMENT_ASSET_ASSET_PRICE_QUANTITY_EXPIRES_AT_HEIGHT_U64_RESERVED_BUYER + "optionalAccount metadata_hash:hash")),
  NftSaleRecordV1: fields("version:u16 offer:NftSaleOfferV1 offer_hash:hash custody:account status:status created_at_height:u64 closed_at_height:optionalU64"),
});
const kinds = ["open", "purchased", "cancelled"];
const custodyPurposes = ["sale", "game_wager", "game_resource"];
function exact(value, names, label) {
  if (!value || typeof value !== "object" || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) throw new TypeError(`${label} must contain exact native fields`);
  const keys = Reflect.ownKeys(value);
  if (keys.length !== names.length || names.some(name => !keys.includes(name))) throw new TypeError(`${label} must contain exact native fields`);
  for (const name of names) {
    const descriptor = Object.getOwnPropertyDescriptor(value, name);
    if (!descriptor?.enumerable || !Object.hasOwn(descriptor, "value")) throw new TypeError(`${label}.${name} must be a native data field`);
  }
}
function unsigned(value, bits, context) {
  if (!((typeof value === "string" && /^(?:0|[1-9][0-9]*)$/u.test(value))
      || (typeof value === "number" && Number.isSafeInteger(value) && value >= 0) || typeof value === "bigint")) {
    throw new TypeError(`${context} must be an exact unsigned integer`);
  }
  const n = BigInt(value);
  if (n < 0n || n >= 1n << BigInt(bits)) throw new RangeError(`${context} exceeds u${bits}`);
  return n;
}
export function createNoritoNftMarketCodecs(h) {
  function encode(name, value, context = name) {
    if (schemas[name]) {
      exact(value, schemas[name].map(([field]) => field), context);
      if ((name === "NftSaleRecordV1" || name === "NftCustodyRecordV1") && Number(value.version) !== 1) throw new TypeError("unknown NFT record version");
      return h.encodeStructValue(schemas[name].map(([field, type]) => [encode(type, value[field], `${context}.${field}`)]));
    }
    switch (name) {
      case "NftCustodyPurposeV1": {
        exact(value, ["kind", "value"], context);
        const tag = custodyPurposes.indexOf(value.kind);
        if (tag < 0 || value.value !== null) throw new TypeError(("unknown" + TEXT_NATIVE_NFT_CUSTODY_PURPOSE));
        return h.encodeU32Value(tag, context);
      }
      case "hash": return h.encodeEscrowIdValue(value, context);
      case "nft": return h.encodeNftIdValue(value, context);
      case "account": return h.encodeAccountIdValue(value, context);
      case "asset": return h.encodeAssetDefinitionIdValue(value, context);
      case "quantity": return h.encodeQuantityValue(value, context);
      case "Metadata": return h.encodeMetadataValue(value, context);
      case "u16": return h.encodeU16Value(unsigned(value, 16, context), context);
      case "u64": return h.encodeU64Value(unsigned(value, 64, context), context);
      case TEXT_OPTIONAL_ACCOUNT: case "optionalU64":
        if (value === undefined) throw new TypeError(`${context} requires an explicit null or value`);
        return h.encodeOptionValue(value, inner => encode(name === TEXT_OPTIONAL_ACCOUNT ? "account" : "u64", inner, context), context);
      case "status": {
        exact(value, ["kind", "value"], context);
        const tag = kinds.indexOf(value.kind);
        if (tag < 0) throw new TypeError("unknown NFT sale status");
        if (tag !== 1 && value.value !== null) throw new TypeError("unit NFT sale status requires explicit null content");
        const bytes = h.encodeU32Value(tag, context);
        return tag === 1 ? Buffer.concat([bytes, h.encodeStructValue([[encode("account", value.value, context)]])]) : bytes;
      }
      default: throw new TypeError(`unknown native NFT value ${name}`);
    }
  }
  function decode(name, payload, context = name) {
    if (schemas[name]) {
      const schema = schemas[name], parts = h.decodeStructFields(payload, context, schema.map(([field]) => field));
      const value = Object.fromEntries(schema.map(([field, type]) => [field, decode(type, parts[field], `${context}.${field}`)]));
      encode(name, value, context); return value;
    }
    switch (name) {
      case "NftCustodyPurposeV1": {
        if (payload.length !== 4) throw new TypeError(("malformed" + TEXT_NATIVE_NFT_CUSTODY_PURPOSE));
        const kind = custodyPurposes[payload.readUInt32LE()];
        if (!kind) throw new TypeError(("unknown" + TEXT_NATIVE_NFT_CUSTODY_PURPOSE));
        return { kind, value: null };
      }
      case "hash": return h.decodeEscrowIdValue(payload, context);
      case "nft": return h.decodeNftIdValue(payload, context);
      case "account": return h.decodeAccountIdValue(payload, context);
      case "asset": return h.decodeAssetDefinitionIdValue(payload, context);
      case "quantity": return h.decodeQuantityValue(payload, context);
      case "Metadata": return h.decodeMetadataValue(payload, context);
      case "u16": return h.decodeU16Value(payload, context);
      case "u64": return h.decodeU64Value(payload, context);
      case TEXT_OPTIONAL_ACCOUNT: case "optionalU64": return h.decodeOptionValue(payload, inner => decode(name === TEXT_OPTIONAL_ACCOUNT ? "account" : "u64", inner, context), context);
      case "status": {
        if (payload.length < 4) throw new TypeError("truncated NFT status");
        const kind = kinds[payload.readUInt32LE()];
        if (!kind || kind !== "purchased" && payload.length !== 4) throw new TypeError("unknown or malformed NFT status");
        if (kind !== "purchased") return { kind, value: null };
        const parts = h.decodeStructFields(payload.subarray(4), context, ["value"]);
        return { kind, value: decode("account", parts.value, context) };
      }
      default: throw new TypeError(`unknown native NFT value ${name}`);
    }
  }
  return Object.freeze({ encode, decode });
}

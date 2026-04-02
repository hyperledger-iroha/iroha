"use strict";

export const CurveFeature = Object.freeze({
  NONE: null,
  ML_DSA: "ml-dsa",
  GOST: "gost",
  SM2: "sm",
  BLS: "bls",
});

export const CurveId = Object.freeze({
  ED25519: 1,
  MLDSA: 2,
  BLS_NORMAL: 3,
  SECP256K1: 4,
  BLS_SMALL: 5,
  GOST_256_A: 10,
  GOST_256_B: 11,
  GOST_256_C: 12,
  GOST_512_A: 13,
  GOST_512_B: 14,
  SM2: 15,
});

export const CURVE_REGISTRY = Object.freeze([
  {
    id: CurveId.ED25519,
    feature: CurveFeature.NONE,
    algorithm: "ed25519",
    aliases: ["ed25519", "ed"],
    publicKeyLength: 32,
    publicKeyMulticodec: 0xed,
  },
  {
    id: CurveId.MLDSA,
    feature: CurveFeature.ML_DSA,
    algorithm: "ml-dsa",
    aliases: ["ml-dsa", "mldsa", "ml_dsa"],
    publicKeyLength: 1952,
    publicKeyMulticodec: 0xee,
  },
  {
    id: CurveId.BLS_NORMAL,
    feature: CurveFeature.BLS,
    algorithm: "bls_normal",
    aliases: ["bls_normal", "bls-normal", "blsnormal"],
    publicKeyLength: 48,
    publicKeyMulticodec: 0xea,
  },
  {
    id: CurveId.SECP256K1,
    feature: CurveFeature.NONE,
    algorithm: "secp256k1",
    aliases: ["secp256k1", "secp-256k1", "secp"],
    publicKeyLength: 33,
    publicKeyMulticodec: 0xe7,
  },
  {
    id: CurveId.BLS_SMALL,
    feature: CurveFeature.BLS,
    algorithm: "bls_small",
    aliases: ["bls_small", "bls-small", "blssmall"],
    publicKeyLength: 96,
    publicKeyMulticodec: 0xeb,
  },
  {
    id: CurveId.GOST_256_A,
    feature: CurveFeature.GOST,
    algorithm: "gost256a",
    aliases: [
      "gost256a",
      "gost-256-a",
      "gost3410-2012-256-paramset-a",
    ],
    publicKeyLength: 64,
    publicKeyMulticodec: 0x1200,
  },
  {
    id: CurveId.GOST_256_B,
    feature: CurveFeature.GOST,
    algorithm: "gost256b",
    aliases: [
      "gost256b",
      "gost-256-b",
      "gost3410-2012-256-paramset-b",
    ],
    publicKeyLength: 64,
    publicKeyMulticodec: 0x1201,
  },
  {
    id: CurveId.GOST_256_C,
    feature: CurveFeature.GOST,
    algorithm: "gost256c",
    aliases: [
      "gost256c",
      "gost-256-c",
      "gost3410-2012-256-paramset-c",
    ],
    publicKeyLength: 64,
    publicKeyMulticodec: 0x1202,
  },
  {
    id: CurveId.GOST_512_A,
    feature: CurveFeature.GOST,
    algorithm: "gost512a",
    aliases: [
      "gost512a",
      "gost-512-a",
      "gost3410-2012-512-paramset-a",
    ],
    publicKeyLength: 128,
    publicKeyMulticodec: 0x1203,
  },
  {
    id: CurveId.GOST_512_B,
    feature: CurveFeature.GOST,
    algorithm: "gost512b",
    aliases: [
      "gost512b",
      "gost-512-b",
      "gost3410-2012-512-paramset-b",
    ],
    publicKeyLength: 128,
    publicKeyMulticodec: 0x1204,
  },
  {
    id: CurveId.SM2,
    feature: CurveFeature.SM2,
    algorithm: "sm2",
    aliases: ["sm2", "sm-2"],
    publicKeyLength: 65,
    publicKeyMulticodec: 0x1306,
  },
]);

const CURVE_NAME_TO_ENTRY = new Map();
const CURVE_ID_TO_ENTRY = new Map();
const CURVE_MULTICODEC_TO_ENTRY = new Map();

for (const entry of CURVE_REGISTRY) {
  CURVE_ID_TO_ENTRY.set(entry.id, entry);
  CURVE_MULTICODEC_TO_ENTRY.set(entry.publicKeyMulticodec, entry);
  CURVE_NAME_TO_ENTRY.set(entry.algorithm, entry);
  for (const alias of entry.aliases) {
    CURVE_NAME_TO_ENTRY.set(alias, entry);
  }
}

export const CURVE_PUBLIC_KEY_LENGTH = new Map(
  CURVE_REGISTRY.map((entry) => [entry.id, entry.publicKeyLength]),
);

export function getCurveEntryById(curveId) {
  return CURVE_ID_TO_ENTRY.get(Number(curveId)) ?? null;
}

export function getCurveEntryByAlgorithm(algorithm) {
  if (algorithm === undefined || algorithm === null) {
    return null;
  }
  return CURVE_NAME_TO_ENTRY.get(String(algorithm).trim().toLowerCase()) ?? null;
}

export function getCurveEntryByPublicKeyMulticodec(multicodec) {
  return CURVE_MULTICODEC_TO_ENTRY.get(Number(multicodec)) ?? null;
}

export function publicKeyMulticodecForCurveId(curveId) {
  return getCurveEntryById(curveId)?.publicKeyMulticodec ?? null;
}

export function canonicalCurveAlgorithm(curveId) {
  return getCurveEntryById(curveId)?.algorithm ?? null;
}

"use strict";

import { ED25519_ALGORITHM, JS_TYPE_STRING } from "./commonLiterals.js";

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
  Object.freeze({
    id: CurveId.ED25519,
    algorithm: ED25519_ALGORITHM,
    publicKeyLength: 32,
    publicKeyMulticodec: 0xed,
  }),
  Object.freeze({
    id: CurveId.MLDSA,
    algorithm: "ml-dsa",
    publicKeyLength: 1952,
    publicKeyMulticodec: 0xee,
  }),
  Object.freeze({
    id: CurveId.BLS_NORMAL,
    algorithm: "bls_normal",
    publicKeyLength: 48,
    publicKeyMulticodec: 0xea,
  }),
  Object.freeze({
    id: CurveId.SECP256K1,
    algorithm: "secp256k1",
    publicKeyLength: 33,
    publicKeyMulticodec: 0xe7,
  }),
  Object.freeze({
    id: CurveId.BLS_SMALL,
    algorithm: "bls_small",
    publicKeyLength: 96,
    publicKeyMulticodec: 0xeb,
  }),
  Object.freeze({
    id: CurveId.GOST_256_A,
    algorithm: "gost256a",
    publicKeyLength: 64,
    publicKeyMulticodec: 0x1200,
  }),
  Object.freeze({
    id: CurveId.GOST_256_B,
    algorithm: "gost256b",
    publicKeyLength: 64,
    publicKeyMulticodec: 0x1201,
  }),
  Object.freeze({
    id: CurveId.GOST_256_C,
    algorithm: "gost256c",
    publicKeyLength: 64,
    publicKeyMulticodec: 0x1202,
  }),
  Object.freeze({
    id: CurveId.GOST_512_A,
    algorithm: "gost512a",
    publicKeyLength: 128,
    publicKeyMulticodec: 0x1203,
  }),
  Object.freeze({
    id: CurveId.GOST_512_B,
    algorithm: "gost512b",
    publicKeyLength: 128,
    publicKeyMulticodec: 0x1204,
  }),
  Object.freeze({
    id: CurveId.SM2,
    algorithm: "sm2",
    publicKeyLength: 65,
    publicKeyMulticodec: 0x1306,
  }),
]);

const CURVE_NAME_TO_ENTRY = new Map();
const CURVE_ID_TO_ENTRY = new Map();
const CURVE_MULTICODEC_TO_ENTRY = new Map();

for (const entry of CURVE_REGISTRY) {
  CURVE_ID_TO_ENTRY.set(entry.id, entry);
  CURVE_MULTICODEC_TO_ENTRY.set(entry.publicKeyMulticodec, entry);
  CURVE_NAME_TO_ENTRY.set(entry.algorithm, entry);
}

export const CURVE_PUBLIC_KEY_LENGTH = new Map(
  CURVE_REGISTRY.map((entry) => [entry.id, entry.publicKeyLength]),
);

export function getCurveEntryById(curveId) {
  return CURVE_ID_TO_ENTRY.get(Number(curveId)) ?? null;
}

export function getCurveEntryByAlgorithm(algorithm) {
  if (typeof algorithm !== JS_TYPE_STRING) {
    return null;
  }
  if (!/^[A-Za-z0-9_-]+$/.test(algorithm)) {
    return null;
  }
  const normalized = algorithm.toLowerCase();
  return CURVE_NAME_TO_ENTRY.get(normalized) ?? null;
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

"use strict";

export const CRYPTO_ALGORITHMS = Object.freeze({
  ED25519: "ed25519",
  SECP256K1: "secp256k1",
  ML_DSA: "ml-dsa",
  BLS_NORMAL: "bls_normal",
  BLS_SMALL: "bls_small",
  GOST_2012_256_A: "gost3410-2012-256-paramset-a",
  GOST_2012_256_B: "gost3410-2012-256-paramset-b",
  GOST_2012_256_C: "gost3410-2012-256-paramset-c",
  GOST_2012_512_A: "gost3410-2012-512-paramset-a",
  GOST_2012_512_B: "gost3410-2012-512-paramset-b",
  SM2: "sm2",
});

export const SUPPORTED_CRYPTO_ALGORITHMS = Object.freeze([
  CRYPTO_ALGORITHMS.ED25519,
  CRYPTO_ALGORITHMS.SECP256K1,
  CRYPTO_ALGORITHMS.BLS_NORMAL,
  CRYPTO_ALGORITHMS.BLS_SMALL,
  CRYPTO_ALGORITHMS.ML_DSA,
  CRYPTO_ALGORITHMS.GOST_2012_256_A,
  CRYPTO_ALGORITHMS.GOST_2012_256_B,
  CRYPTO_ALGORITHMS.GOST_2012_256_C,
  CRYPTO_ALGORITHMS.GOST_2012_512_A,
  CRYPTO_ALGORITHMS.GOST_2012_512_B,
  CRYPTO_ALGORITHMS.SM2,
]);

const SUPPORTED_CRYPTO_ALGORITHM_SET = new Set(SUPPORTED_CRYPTO_ALGORITHMS);

export function normalizeCryptoAlgorithm(
  algorithm = CRYPTO_ALGORITHMS.ED25519,
) {
  if (
    typeof algorithm !== "string" ||
    !SUPPORTED_CRYPTO_ALGORITHM_SET.has(algorithm)
  ) {
    throw new Error(`unsupported crypto algorithm: ${algorithm}`);
  }
  return algorithm;
}

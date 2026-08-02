export const AUTHENTICATED_BLOCK_PROOFS_VERSION_V1 = 1;
export const AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1 = 32 * 1024 * 1024;
export const AUTHENTICATED_BLOCK_PROOFS_MAX_FINALITY_PROOF_BYTES_V1 = 9 * 1024 * 1024;
export const AUTHENTICATED_BLOCK_PROOFS_MAX_PROOF_BYTES_V1 = 16 * 1024 * 1024;

/**
 * Browser builds fail closed until a digest-pinned Rust finality-verifier WASM
 * artifact is shipped. Pure JavaScript Merkle checks cannot authenticate a
 * finality anchor.
 */
export async function verifyAuthenticatedBlockProofsV1() {
  const error = new Error(
    "authenticated BlockProofs verification requires the native Rust verifier; no digest-pinned browser finality-verifier WASM is shipped",
  );
  Object.defineProperty(error, "code", {
    value: "ERR_IROHA_AUTHENTICATED_BLOCK_PROOFS_UNAVAILABLE",
    enumerable: true,
  });
  throw error;
}

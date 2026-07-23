/** Fixed byte length of the canonical IVM program header. */
export const IVM_PROGRAM_HEADER_LENGTH: 49;

/** Default ledger limit for one complete deployed IVM artifact (4 MiB). */
export const IVM_ARTIFACT_MAX_BYTES: 4194304;

/** Independently verifiable identities for one complete IVM artifact. */
export interface IvmArtifactHashes {
  /** Ledger/Core BLAKE2b-256 identity of the artifact body. */
  codeHashHex: string;
  /** SHA-256 identity committing to every artifact header and body byte. */
  artifactSha256Hex: string;
}

/**
 * Compute the ledger/Core code identity and full-artifact SHA-256 identity.
 * This subpath is browser-safe and does not require ambient Node declarations.
 */
export function computeIvmArtifactHashes(
  artifact: Uint8Array | ArrayBuffer | ArrayBufferView,
): IvmArtifactHashes;

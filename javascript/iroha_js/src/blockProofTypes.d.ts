export interface ToriiBlockMerkleProof {
  readonly leaf_index: number;
  readonly audit_path: ReadonlyArray<string | null>;
}

export interface ToriiBlockReceiptProof {
  readonly leaf: string;
  readonly proof: ToriiBlockMerkleProof;
}

export interface ToriiBlockMerkleCommitment {
  readonly root: string;
  readonly leaf_count: string;
}

export interface ToriiBlockProofTransferSmtWitness {
  readonly root_before: string;
  readonly root_after: string;
  readonly path_bits: ReadonlyArray<number>;
  readonly siblings: ReadonlyArray<string>;
}

export interface ToriiBlockProofTransferDeltaTranscript {
  readonly from_account: string;
  readonly to_account: string;
  readonly asset_definition: string;
  readonly amount: string;
  readonly from_balance_before: string;
  readonly from_balance_after: string;
  readonly to_balance_before: string;
  readonly to_balance_after: string;
  readonly from_smt_witness: ToriiBlockProofTransferSmtWitness;
  readonly to_smt_witness: ToriiBlockProofTransferSmtWitness;
}

export interface ToriiBlockProofTransferTranscript {
  readonly batch_hash: string;
  readonly deltas: ReadonlyArray<ToriiBlockProofTransferDeltaTranscript>;
  readonly authority_digest: string;
  readonly poseidon_preimage_digest: string | null;
}

export interface ToriiBlockProofs {
  readonly block_height: string;
  readonly block_hash: string;
  readonly executed_block_wire_hash: string;
  readonly entry_hash: string;
  readonly entry_commitment: ToriiBlockMerkleCommitment;
  readonly entry_proof: ToriiBlockReceiptProof;
  readonly result_commitment: ToriiBlockMerkleCommitment;
  readonly result_proof: ToriiBlockReceiptProof;
  readonly fastpq_transcripts: Readonly<
    Record<string, ReadonlyArray<ToriiBlockProofTransferTranscript>>
  >;
}

/**
 * Structural anchor for local BlockProofs consistency checks.
 *
 * The SDK does not authenticate this value or block finality. It must come
 * from an independently authenticated block and must never be copied from the
 * ToriiBlockProofs response being checked.
 */
export interface ToriiBlockProofTrustedAnchor {
  readonly block_height: string;
  readonly block_hash: string;
  readonly executed_block_wire_hash: string;
  readonly entry_hash: string;
  readonly entry_index: number;
  readonly entry_commitment: ToriiBlockMerkleCommitment;
  readonly result_commitment: ToriiBlockMerkleCommitment;
  readonly fastpq_transcripts: ToriiBlockProofs["fastpq_transcripts"];
}

export interface ToriiBlockProofVerification {
  /** Consistency with the supplied anchor; this is not a finality verdict. */
  readonly valid: boolean;
  readonly anchor_matches: boolean;
  readonly entry_hash_matches: boolean;
  readonly entry_proof_valid: boolean;
  readonly result_pair_consistent: boolean;
  readonly result_proof_valid: boolean;
}

/** First-release native authenticated BlockProofs verifier version. */
export const AUTHENTICATED_BLOCK_PROOFS_VERSION_V1: 1;
/** Maximum exact executed SignedBlockWire bytes accepted by the native verifier. */
export const AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1: 33554432;
/** Maximum canonical Norito bytes accepted for one BridgeFinalityProof. */
export const AUTHENTICATED_BLOCK_PROOFS_MAX_FINALITY_PROOF_BYTES_V1: 9437184;
/** Maximum canonical Norito bytes accepted for one BlockProofs response. */
export const AUTHENTICATED_BLOCK_PROOFS_MAX_PROOF_BYTES_V1: 16777216;

export interface AuthenticatedBlockProofInputV1 {
  readonly version: 1;
  /** Application-pinned exact genesis-derived NetworkId; this must not be sourced from the response. */
  readonly networkId: string;
  /** Application-pinned marked 32-byte HeightContextId. */
  readonly trustedContextId: ArrayBufferView | ArrayBuffer;
  /** Application-selected marked 32-byte transaction entrypoint hash. */
  readonly expectedEntryHash: ArrayBufferView | ArrayBuffer;
  /**
   * Optional last verified BridgeFinalityProof. When present, the target proof
   * must be its immediate cryptographic successor.
   */
  readonly previousFinalityProofNorito?:
    | ArrayBufferView
    | ArrayBuffer
    | null;
  /** Canonical Norito BridgeFinalityProof for the target block. */
  readonly finalityProofNorito: ArrayBufferView | ArrayBuffer;
  /** Exact canonical executed SignedBlockWire for the target block. */
  readonly executedBlockWire: ArrayBufferView | ArrayBuffer;
  /** Canonical Norito BlockProofs response returned by Torii. */
  readonly blockProofsNorito: ArrayBufferView | ArrayBuffer;
}

export interface AuthenticatedBlockProofVerdictV1 {
  /** Finality is authenticated whenever a verdict resolves; this additionally covers BlockProofs. */
  readonly valid: boolean;
  readonly code: "valid" | "block_proofs_mismatch";
  readonly blockHeight: string;
  readonly blockHashHex: string;
  readonly executedBlockWireHashHex: string;
  readonly entryHashHex: string;
  /** Verified context to retain alongside the accepted finality proof for successor state. */
  readonly heightContextIdHex: string;
}

/**
 * Verify Torii BlockProofs through the native Rust Sumeragi-v2 finality path.
 *
 * The promise rejects on malformed, non-canonical, wrong-chain, wrong-context,
 * stale/skipped, or cryptographically invalid finality material. A valid
 * finality chain carrying inconsistent Merkle/result/transcript proofs resolves
 * to a verdict whose `valid` field is false.
 */
export function verifyAuthenticatedBlockProofsV1(
  input: Readonly<AuthenticatedBlockProofInputV1>,
): Promise<Readonly<AuthenticatedBlockProofVerdictV1>>;

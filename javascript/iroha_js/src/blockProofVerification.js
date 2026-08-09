import { Buffer } from "buffer";
import { blake2b256 } from "./blake2b.js";

const BLOCK_MERKLE_MAX_HEIGHT = 32;
const BLOCK_MERKLE_LEAF_NODE_DOMAIN = Buffer.from(
  "iroha:merkle:leaf:v1\0",
  "utf8",
);
const BLOCK_MERKLE_INTERNAL_NODE_DOMAIN = Buffer.from(
  "iroha:merkle:internal:v1\0",
  "utf8",
);

function isPlainObject(value) {
  return Object.prototype.toString.call(value) === "[object Object]";
}

export function createBlockProofVerification(hashBytes) {
  function blockProofHashBytes(value, context) {
    const bytes = Buffer.from(hashBytes(value, context));
    if ((bytes[bytes.length - 1] & 1) !== 1) {
      throw new Error(`${context} does not carry Iroha's hash marker bit`);
    }
    return bytes;
  }

  function blockProofHashesEqual(left, right, context) {
    return blockProofHashBytes(left, `${context}.left`).equals(
      blockProofHashBytes(right, `${context}.right`),
    );
  }

  function blockMerkleCommitmentsEqual(left, right, context) {
    const leftParts = blockMerkleCommitmentParts(left, `${context}.left`);
    const rightParts = blockMerkleCommitmentParts(right, `${context}.right`);
    return leftParts !== null &&
      rightParts !== null &&
      leftParts.leafCount === rightParts.leafCount &&
      leftParts.root.equals(rightParts.root) &&
      leftParts.depth === rightParts.depth;
  }

  function blockProofValuesEqual(left, right, depth = 0) {
    if (depth > 64) return false;
    if (Array.isArray(left) || Array.isArray(right)) {
      if (!Array.isArray(left) || !Array.isArray(right) || left.length !== right.length) {
        return false;
      }
      return left.every((value, index) =>
        blockProofValuesEqual(value, right[index], depth + 1));
    }
    if (isPlainObject(left) || isPlainObject(right)) {
      if (!isPlainObject(left) || !isPlainObject(right)) return false;
      const leftKeys = Object.keys(left).sort();
      const rightKeys = Object.keys(right).sort();
      if (
        leftKeys.length !== rightKeys.length ||
        leftKeys.some((key, index) => key !== rightKeys[index])
      ) return false;
      return leftKeys.every((key) =>
        blockProofValuesEqual(left[key], right[key], depth + 1));
    }
    return left === right;
  }

  function blockMerkleCommitmentParts(commitment, context = "Merkle commitment") {
    if (!isPlainObject(commitment)) return null;
    const root = blockProofHashBytes(commitment.root, `${context}.root`);
    let leafCount;
    try {
      leafCount = BigInt(commitment.leaf_count);
    } catch {
      return null;
    }
    if (
      leafCount <= 0n ||
      leafCount > (1n << BigInt(BLOCK_MERKLE_MAX_HEIGHT))
    ) {
      return null;
    }
    let depth = 0;
    for (let width = leafCount; width > 1n; width = (width + 1n) >> 1n) {
      depth += 1;
    }
    return { root, leafCount, depth };
  }

  /** Verify one Iroha block Merkle audit path against an exact root/count commitment. */
  function verifyBlockMerkleProof(leaf, proof, commitment) {
    try {
      const leafBytes = blockProofHashBytes(leaf, "Merkle proof leaf");
      const commitmentParts = blockMerkleCommitmentParts(commitment);
      if (commitmentParts === null) return false;
      const { root: rootBytes, leafCount, depth } = commitmentParts;
      if (!isPlainObject(proof)) return false;
      const leafIndex = proof.leaf_index;
      const auditPath = proof.audit_path;
      if (
        !Number.isInteger(leafIndex) ||
        leafIndex < 0 ||
        leafIndex > 0xffff_ffff ||
        !Array.isArray(auditPath) ||
        auditPath.length !== depth
      ) {
        return false;
      }
      if (BigInt(leafIndex) >= leafCount) return false;

      let index = BigInt(leafIndex);
      let width = leafCount;
      let accumulator = Buffer.from(blake2b256(
        Buffer.concat([BLOCK_MERKLE_LEAF_NODE_DOMAIN, leafBytes]),
      ));
      accumulator[31] |= 1;
      for (let level = 0; level < auditPath.length; level += 1) {
        const rawSibling = auditPath[level];
        const sibling = rawSibling === null
          ? null
          : blockProofHashBytes(rawSibling, `Merkle proof audit_path[${level}]`);
        const currentIsRight = (index & 1n) === 1n;
        const siblingMustExist = currentIsRight || index + 1n < width;
        if (siblingMustExist !== (sibling !== null)) return false;
        if (sibling === null) {
          index >>= 1n;
          width = (width + 1n) >> 1n;
          continue;
        }
        const parentInput = currentIsRight
          ? Buffer.concat([BLOCK_MERKLE_INTERNAL_NODE_DOMAIN, sibling, accumulator])
          : Buffer.concat([BLOCK_MERKLE_INTERNAL_NODE_DOMAIN, accumulator, sibling]);
        accumulator = Buffer.from(blake2b256(parentInput));
        accumulator[31] |= 1;
        index >>= 1n;
        width = (width + 1n) >> 1n;
      }
      return accumulator.equals(rootBytes);
    } catch {
      return false;
    }
  }

  /**
   * Check `BlockProofs` Merkle consistency against a caller-authenticated anchor.
   * This pure JavaScript helper authenticates neither that structural anchor nor
   * block finality. A proof response is never its own source of trust, and a
   * missing or malformed anchor fails closed.
   */
  function verifyBlockProofs(proofs, trustedAnchor) {
    const invalid = {
      valid: false,
      anchor_matches: false,
      entry_hash_matches: false,
      entry_proof_valid: false,
      result_pair_consistent: false,
      result_proof_valid: false,
    };
    if (
      !isPlainObject(proofs) ||
      !isPlainObject(proofs.entry_proof) ||
      !isPlainObject(proofs.result_proof) ||
      !isPlainObject(proofs.fastpq_transcripts) ||
      !isPlainObject(trustedAnchor) ||
      !isPlainObject(trustedAnchor.fastpq_transcripts)
    ) return invalid;
    try {
      const blockHeightMatches = BigInt(proofs.block_height) === BigInt(trustedAnchor.block_height);
      const blockHashMatches = blockProofHashesEqual(
        proofs.block_hash,
        trustedAnchor.block_hash,
        "BlockProofs block hash anchor",
      );
      const executedWireMatches = blockProofHashesEqual(
        proofs.executed_block_wire_hash,
        trustedAnchor.executed_block_wire_hash,
        "BlockProofs executed block wire anchor",
      );
      const entryCommitmentMatches = blockMerkleCommitmentsEqual(
        proofs.entry_commitment,
        trustedAnchor.entry_commitment,
        "BlockProofs entry commitment anchor",
      );
      const resultCommitmentMatches = blockMerkleCommitmentsEqual(
        proofs.result_commitment,
        trustedAnchor.result_commitment,
        "BlockProofs result commitment anchor",
      );
      const trustedEntryCommitment = blockMerkleCommitmentParts(
        trustedAnchor.entry_commitment,
        "BlockProofs trusted entry commitment",
      );
      const trustedResultCommitment = blockMerkleCommitmentParts(
        trustedAnchor.result_commitment,
        "BlockProofs trusted result commitment",
      );
      const leafCountsAlign = trustedEntryCommitment !== null &&
        trustedResultCommitment !== null &&
        trustedEntryCommitment.leafCount === trustedResultCommitment.leafCount;
      const fastpqTranscriptsMatch = blockProofValuesEqual(
        proofs.fastpq_transcripts,
        trustedAnchor.fastpq_transcripts,
      );
      const anchoredEntryIndex = trustedAnchor.entry_index;
      const entryIndexMatches = Number.isInteger(anchoredEntryIndex) &&
        anchoredEntryIndex >= 0 &&
        anchoredEntryIndex <= 0xffff_ffff &&
        isPlainObject(proofs.entry_proof.proof) &&
        proofs.entry_proof.proof.leaf_index === anchoredEntryIndex;
      const anchorMatches = blockHeightMatches &&
        blockHashMatches &&
        executedWireMatches &&
        entryCommitmentMatches &&
        resultCommitmentMatches &&
        leafCountsAlign &&
        fastpqTranscriptsMatch &&
        entryIndexMatches;
      const entryHashMatches =
        blockProofHashesEqual(
          proofs.entry_hash,
          proofs.entry_proof.leaf,
          "BlockProofs entry hash",
        ) &&
        blockProofHashesEqual(
          proofs.entry_hash,
          trustedAnchor.entry_hash,
          "BlockProofs requested entry hash anchor",
        );
      const entryProofValid = verifyBlockMerkleProof(
        proofs.entry_proof.leaf,
        proofs.entry_proof.proof,
        trustedAnchor.entry_commitment,
      );
      const resultIndexMatches = isPlainObject(proofs.result_proof.proof) &&
        proofs.result_proof.proof.leaf_index === proofs.entry_proof.proof.leaf_index;
      const resultPairConsistent =
        resultCommitmentMatches &&
        leafCountsAlign &&
        resultIndexMatches;
      const resultProofValid = resultPairConsistent && verifyBlockMerkleProof(
        proofs.result_proof.leaf,
        proofs.result_proof.proof,
        trustedAnchor.result_commitment,
      );
      return {
        valid:
          anchorMatches &&
          entryHashMatches &&
          entryProofValid &&
          resultPairConsistent &&
          resultProofValid,
        anchor_matches: anchorMatches,
        entry_hash_matches: entryHashMatches,
        entry_proof_valid: entryProofValid,
        result_pair_consistent: resultPairConsistent,
        result_proof_valid: resultProofValid,
      };
    } catch {
      return invalid;
    }
  }

  return { verifyBlockMerkleProof, verifyBlockProofs };
}

// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

/**
 * @title ISccpMessageVerifier
 * @dev External verifier interface for SCCP message-proof wrappers.
 *
 * The verifier is responsible for validating `proofBytes` against the supplied
 * fixed-width `publicInputs` words and `statementHash`, then returning the
 * canonical message identity and statement anchors if the proof is valid.
 */
interface ISccpMessageVerifier {
    function verifySccpMessageProof(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes32 destinationBindingHash
    )
        external
        view
        returns (
            bytes32 messageId,
            uint32 sourceDomain,
            bytes32 commitmentRoot
        );
}

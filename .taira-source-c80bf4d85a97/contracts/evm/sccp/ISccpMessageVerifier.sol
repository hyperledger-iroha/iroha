// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.7.4;

/**
 * @title ISccpMessageVerifier
 * @dev External verifier interface for SCCP message-proof wrappers.
 *
 * The verifier is responsible for validating `proofBytes` against the supplied
 * fixed-width `publicInputs` words and `statementHash`, then returning the
 * canonical message identity and statement anchors if the proof is valid.
 */
interface ISccpMessageVerifier {
    /** Hash of the audited semantic proof profile implemented by the fixed key. */
    function semanticProofProfileHash() external view returns (bytes32);

    /** Hash of the governed SORA finality checkpoint and validator-set anchor. */
    function soraFinalityAnchorHash() external view returns (bytes32);

    function verifySccpMessageProof(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes32 destinationBindingHash,
        bytes32 routeConfigurationHash
    )
        external
        view
        returns (
            bytes32 messageId,
            uint32 sourceDomain,
            bytes32 commitmentRoot
        );
}

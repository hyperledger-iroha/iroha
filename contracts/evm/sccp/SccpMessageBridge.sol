// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

import "./ISccpMessageVerifier.sol";
import "./Ownable.sol";

/**
 * @title SccpMessageBridge
 * @dev Replay-protected SCCP message-proof submission wrapper for EVM lanes.
 *
 * This contract binds a deployment to one concrete verifier backend / proof
 * family pair. The actual proof system lives behind `ISccpMessageVerifier`.
 */
contract SccpMessageBridge is Ownable {
    ISccpMessageVerifier public verifier;
    bytes32 public verifierBackendHash;
    bytes32 public proofFamilyHash;
    bytes32 public networkId;
    mapping(bytes32 => bool) public usedMessageProofs;

    event VerifierConfigured(
        address indexed verifier,
        bytes32 verifierBackendHash,
        bytes32 proofFamilyHash
    );

    event MessageProofAccepted(
        bytes32 indexed messageId,
        uint32 indexed sourceDomain,
        bytes32 commitmentRoot,
        bytes32 verifierBackendHash,
        bytes32 proofFamilyHash,
        bytes32 networkId
    );

    constructor(
        address verifierAddress,
        string memory verifierBackendKey,
        string memory proofFamily,
        bytes32 configuredNetworkId
    ) {
        networkId = configuredNetworkId;
        _configureVerifier(verifierAddress, verifierBackendKey, proofFamily);
    }

    function configureVerifier(
        address verifierAddress,
        string memory verifierBackendKey,
        string memory proofFamily
    ) public onlyOwner {
        _configureVerifier(verifierAddress, verifierBackendKey, proofFamily);
    }

    function submitSccpMessageProof(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash
    ) external returns (bytes32 messageId) {
        require(address(verifier) != address(0), "Verifier is not configured");

        uint32 sourceDomain;
        bytes32 commitmentRoot;
        (messageId, sourceDomain, commitmentRoot) = verifier
            .verifySccpMessageProof(proofBytes, publicInputs, statementHash);

        require(messageId != bytes32(0), "Verifier returned empty message id");
        require(
            !usedMessageProofs[messageId],
            "Message proof already used"
        );

        usedMessageProofs[messageId] = true;

        emit MessageProofAccepted(
            messageId,
            sourceDomain,
            commitmentRoot,
            verifierBackendHash,
            proofFamilyHash,
            networkId
        );
    }

    function _configureVerifier(
        address verifierAddress,
        string memory verifierBackendKey,
        string memory proofFamily
    ) internal {
        require(verifierAddress != address(0), "Verifier address is required");

        verifier = ISccpMessageVerifier(verifierAddress);
        verifierBackendHash = keccak256(bytes(verifierBackendKey));
        proofFamilyHash = keccak256(bytes(proofFamily));

        emit VerifierConfigured(
            verifierAddress,
            verifierBackendHash,
            proofFamilyHash
        );
    }
}

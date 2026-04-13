// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

import "./ISccpMessageVerifier.sol";

/**
 * @title SccpMessageBridge
 * @dev Replay-protected SCCP message-proof submission wrapper for EVM lanes.
 *
 * This contract binds a deployment to one concrete verifier backend / proof
 * family pair. The actual proof system lives behind `ISccpMessageVerifier`.
 */
contract SccpMessageBridge {
    bytes32 private constant DESTINATION_BINDING_DOMAIN_SEPARATOR =
        keccak256("iroha:sccp:evm-destination-binding:v1");
    ISccpMessageVerifier public immutable verifier;
    bytes32 public immutable verifierBackendHash;
    bytes32 public immutable proofFamilyHash;
    bytes32 public immutable networkId;
    uint32 public immutable expectedSourceDomain;
    uint32 public immutable expectedTargetDomain;
    mapping(bytes32 => bool) public usedMessageProofs;

    event VerifierBound(
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
        bytes32 configuredNetworkId,
        uint32 configuredSourceDomain,
        uint32 configuredTargetDomain
    ) {
        require(verifierAddress != address(0), "Verifier address is required");

        bytes32 backendHash = keccak256(bytes(verifierBackendKey));
        bytes32 familyHash = keccak256(bytes(proofFamily));

        verifier = ISccpMessageVerifier(verifierAddress);
        verifierBackendHash = backendHash;
        proofFamilyHash = familyHash;
        networkId = configuredNetworkId;
        expectedSourceDomain = configuredSourceDomain;
        expectedTargetDomain = configuredTargetDomain;

        emit VerifierBound(
            verifierAddress,
            backendHash,
            familyHash
        );
    }

    function submitSccpMessageProof(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash
    ) external returns (bytes32 messageId) {
        bytes32 destinationBindingHash = keccak256(
            abi.encode(
                DESTINATION_BINDING_DOMAIN_SEPARATOR,
                verifierBackendHash,
                proofFamilyHash,
                networkId,
                uint256(expectedSourceDomain),
                uint256(expectedTargetDomain),
                address(verifier),
                address(this)
            )
        );
        uint32 sourceDomain;
        bytes32 commitmentRoot;
        (messageId, sourceDomain, commitmentRoot) = verifier
            .verifySccpMessageProof(
                proofBytes,
                publicInputs,
                statementHash,
                destinationBindingHash
            );

        require(messageId != bytes32(0), "Verifier returned empty message id");
        require(sourceDomain == expectedSourceDomain, "Unexpected source domain");
        require(publicInputs[0] == messageId, "Public inputs message id mismatch");
        require(
            publicInputs[2] == _abiWordU32(expectedTargetDomain),
            "Unexpected target domain"
        );
        require(
            publicInputs[3] == commitmentRoot,
            "Public inputs commitment root mismatch"
        );
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

    function _abiWordU32(uint32 value) internal pure returns (bytes32 out) {
        out = bytes32(uint256(value));
    }
}

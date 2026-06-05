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
    uint32 private constant SCCP_DOMAIN_SORA = 0;
    uint32 private constant SCCP_DOMAIN_ETH = 1;
    uint32 private constant SCCP_DOMAIN_BSC = 2;
    bytes32 private constant ETH_MAINNET_NETWORK_ID = bytes32(uint256(1));
    bytes32 private constant BSC_MAINNET_NETWORK_ID = bytes32(uint256(56));
    bytes32 private constant BSC_TESTNET_NETWORK_ID = bytes32(uint256(97));
    bytes32 private constant DESTINATION_BINDING_DOMAIN_SEPARATOR =
        keccak256("iroha:sccp:evm-destination-binding:v1");
    bytes32 private constant PRODUCTION_GROTH16_BACKEND_HASH =
        keccak256("evm-groth16-bn254-v1");
    bytes32 private constant STARK_FRI_PROOF_FAMILY_HASH =
        keccak256("stark-fri-v1");
    bytes32 private constant EMPTY_ACCOUNT_CODE_HASH = keccak256("");
    ISccpMessageVerifier public immutable verifier;
    bytes32 public immutable verifierCodeHash;
    bytes32 public immutable verifierKeyHash;
    bytes32 public immutable verifierBackendHash;
    bytes32 public immutable proofFamilyHash;
    bytes32 public immutable networkId;
    uint32 public immutable expectedSourceDomain;
    uint32 public immutable expectedTargetDomain;
    mapping(bytes32 => bool) public usedMessageProofs;

    event VerifierBound(
        address indexed verifier,
        bytes32 verifierCodeHash,
        bytes32 verifierKeyHash,
        bytes32 verifierBackendHash,
        bytes32 proofFamilyHash
    );

    event MessageProofAccepted(
        bytes32 indexed messageId,
        uint32 indexed sourceDomain,
        bytes32 commitmentRoot,
        bytes32 statementHash,
        bytes32 destinationBindingHash,
        bytes32 verifierBackendHash,
        bytes32 proofFamilyHash,
        bytes32 networkId
    );

    constructor(
        address verifierAddress,
        bytes32 expectedVerifierCodeHash,
        bytes32 expectedVerifierKeyHash,
        string memory verifierBackendKey,
        string memory proofFamily,
        bytes32 configuredNetworkId,
        uint32 configuredSourceDomain,
        uint32 configuredTargetDomain
    ) {
        require(verifierAddress != address(0), "Verifier address is required");
        require(
            bytes(verifierBackendKey).length != 0,
            "Verifier backend key is required"
        );
        require(bytes(proofFamily).length != 0, "Proof family is required");
        require(configuredNetworkId != bytes32(0), "Network id is required");
        require(configuredTargetDomain != 0, "Target domain is required");
        require(
            configuredSourceDomain == SCCP_DOMAIN_SORA,
            "Source domain must be SORA"
        );
        require(
            configuredTargetDomain == SCCP_DOMAIN_ETH ||
                configuredTargetDomain == SCCP_DOMAIN_BSC,
            "Target domain must be ETH or BSC"
        );
        if (configuredTargetDomain == SCCP_DOMAIN_ETH) {
            require(
                configuredNetworkId == ETH_MAINNET_NETWORK_ID,
                "Network id must be ETH mainnet"
            );
        }
        if (configuredTargetDomain == SCCP_DOMAIN_BSC) {
            require(
                configuredNetworkId == BSC_MAINNET_NETWORK_ID ||
                    configuredNetworkId == BSC_TESTNET_NETWORK_ID,
                "Network id must be BSC mainnet or testnet"
            );
        }
        require(
            configuredSourceDomain != configuredTargetDomain,
            "Source and target domains must differ"
        );

        bytes32 backendHash = keccak256(bytes(verifierBackendKey));
        bytes32 familyHash = keccak256(bytes(proofFamily));
        require(
            backendHash == PRODUCTION_GROTH16_BACKEND_HASH,
            "Unsupported verifier backend"
        );
        require(
            familyHash == STARK_FRI_PROOF_FAMILY_HASH,
            "Proof family must be stark-fri-v1"
        );
        bytes32 actualVerifierCodeHash = _codeHash(verifierAddress);
        require(
            expectedVerifierCodeHash != bytes32(0),
            "Verifier code hash is required"
        );
        require(
            actualVerifierCodeHash != bytes32(0) &&
                actualVerifierCodeHash != EMPTY_ACCOUNT_CODE_HASH,
            "Verifier code is required"
        );
        require(
            actualVerifierCodeHash == expectedVerifierCodeHash,
            "Verifier code hash mismatch"
        );
        require(
            expectedVerifierKeyHash != bytes32(0),
            "Verifier key hash is required"
        );
        require(
            _verifyingKeyHash(verifierAddress) == expectedVerifierKeyHash,
            "Verifier key hash mismatch"
        );

        verifier = ISccpMessageVerifier(verifierAddress);
        verifierCodeHash = actualVerifierCodeHash;
        verifierKeyHash = expectedVerifierKeyHash;
        verifierBackendHash = backendHash;
        proofFamilyHash = familyHash;
        networkId = configuredNetworkId;
        expectedSourceDomain = configuredSourceDomain;
        expectedTargetDomain = configuredTargetDomain;

        emit VerifierBound(
            verifierAddress,
            actualVerifierCodeHash,
            expectedVerifierKeyHash,
            backendHash,
            familyHash
        );
    }

    function submitSccpMessageProof(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash
    ) external returns (bytes32 messageId) {
        require(statementHash != bytes32(0), "Statement hash is required");
        require(publicInputs[0] != bytes32(0), "Message id is required");
        require(publicInputs[1] != bytes32(0), "Payload hash is required");
        require(
            publicInputs[2] == _abiWordU32(expectedTargetDomain),
            "Unexpected target domain"
        );
        require(publicInputs[3] != bytes32(0), "Commitment root is required");
        require(publicInputs[4] != bytes32(0), "Finality height is required");
        require(publicInputs[5] != bytes32(0), "Finality block hash is required");

        bytes32 bindingHash = _destinationBindingHash();
        uint32 sourceDomain;
        bytes32 commitmentRoot;
        (messageId, sourceDomain, commitmentRoot) = verifier
            .verifySccpMessageProof(
                proofBytes,
                publicInputs,
                statementHash,
                bindingHash
            );

        require(messageId != bytes32(0), "Verifier returned empty message id");
        require(sourceDomain == expectedSourceDomain, "Unexpected source domain");
        require(publicInputs[0] == messageId, "Public inputs message id mismatch");
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
            statementHash,
            bindingHash,
            verifierBackendHash,
            proofFamilyHash,
            networkId
        );
    }

    function destinationBindingHash() external view returns (bytes32) {
        return _destinationBindingHash();
    }

    function _destinationBindingHash() internal view returns (bytes32) {
        return keccak256(
            abi.encode(
                DESTINATION_BINDING_DOMAIN_SEPARATOR,
                verifierBackendHash,
                proofFamilyHash,
                networkId,
                uint256(expectedSourceDomain),
                uint256(expectedTargetDomain),
                address(verifier),
                address(this),
                verifierCodeHash,
                verifierKeyHash
            )
        );
    }

    function _abiWordU32(uint32 value) internal pure returns (bytes32 out) {
        out = bytes32(uint256(value));
    }

    function _codeHash(address account) private view returns (bytes32 codeHash) {
        assembly {
            codeHash := extcodehash(account)
        }
    }

    function _verifyingKeyHash(address verifierAddress)
        private
        view
        returns (bytes32 keyHash)
    {
        (bool success, bytes memory data) = verifierAddress.staticcall(
            abi.encodeWithSignature("verifyingKeyHash()")
        );
        require(success && data.length == 32, "Verifier key hash unavailable");
        keyHash = abi.decode(data, (bytes32));
    }
}

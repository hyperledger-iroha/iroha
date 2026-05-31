// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

import "../../evm/sccp/SccpGroth16Bn254MessageVerifier.sol";

/**
 * @title SccpTronGroth16Bn254MessageVerifier
 * @dev TRON/TVM deployment entrypoint for the SCCP BN254 Groth16 verifier.
 *
 * TVM is EVM-compatible for Solidity contracts and supports the altbn128
 * operations used by the inherited verifier. This wrapper keeps the deployed
 * contract name and artifact path TRON-specific while adding replay-protected
 * submission state around the exact SCCP public-signal, verifying-key, and
 * proof-validation logic shared with EVM-compatible lanes.
 */
contract SccpTronGroth16Bn254MessageVerifier is SccpGroth16Bn254MessageVerifier {
    uint256 private constant GROTH16_PROOF_VERSION = 1;
    uint256 private constant GROTH16_PROOF_ABI_WORD_COUNT = 12;
    uint256 private constant GROTH16_PROOF_ABI_BYTE_LENGTH =
        GROTH16_PROOF_ABI_WORD_COUNT * 32;
    uint32 private constant SCCP_DOMAIN_SORA = 0;
    uint32 private constant SCCP_DOMAIN_TRON = 5;

    bytes32 private constant DESTINATION_BINDING_DOMAIN_SEPARATOR =
        keccak256("iroha:sccp:tron-destination-binding:v1");
    bytes32 private constant TRON_GROTH16_BACKEND_HASH =
        keccak256("tron-groth16-bn254-v1");
    bytes32 private constant STARK_FRI_PROOF_FAMILY_HASH =
        keccak256("stark-fri-v1");

    bytes32 public immutable verifierKeyHash;
    bytes32 public immutable verifierBackendHash;
    bytes32 public immutable proofFamilyHash;
    bytes32 public immutable networkId;
    uint32 public immutable expectedSourceDomain;
    uint32 public immutable expectedTargetDomain;
    mapping(bytes32 => bool) public usedMessageProofs;

    event VerifierBound(
        address indexed verifier,
        bytes32 verifierKeyHash,
        bytes32 verifierBackendHash,
        bytes32 proofFamilyHash
    );

    event DestinationBindingConfigured(
        bytes32 indexed destinationBindingHash,
        bytes32 verifierCodeHash,
        bytes32 verifierKeyHash,
        bytes32 networkId,
        uint32 indexed sourceDomain,
        uint32 indexed targetDomain
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
        uint256[2] memory configuredAlpha1,
        uint256[4] memory configuredBeta2,
        uint256[4] memory configuredGamma2,
        uint256[4] memory configuredDelta2,
        uint256[] memory configuredIc,
        bytes32 expectedVerifierKeyHash,
        string memory proofFamily,
        bytes32 configuredNetworkId,
        uint32 configuredSourceDomain,
        uint32 configuredTargetDomain
    )
        SccpGroth16Bn254MessageVerifier(
            configuredAlpha1,
            configuredBeta2,
            configuredGamma2,
            configuredDelta2,
            configuredIc
        )
    {
        require(
            expectedVerifierKeyHash != bytes32(0),
            "Verifier key hash is required"
        );
        require(
            verifyingKeyHash() == expectedVerifierKeyHash,
            "Verifier key hash mismatch"
        );
        require(bytes(proofFamily).length != 0, "Proof family is required");
        require(configuredNetworkId != bytes32(0), "Network id is required");
        require(
            configuredTargetDomain == SCCP_DOMAIN_TRON,
            "Target domain must be TRON"
        );
        require(
            configuredSourceDomain == SCCP_DOMAIN_SORA,
            "Source domain must be SORA"
        );
        require(
            configuredSourceDomain != configuredTargetDomain,
            "Source and target domains must differ"
        );

        bytes32 configuredProofFamilyHash = keccak256(bytes(proofFamily));
        require(
            configuredProofFamilyHash == STARK_FRI_PROOF_FAMILY_HASH,
            "Proof family must be stark-fri-v1"
        );

        verifierKeyHash = expectedVerifierKeyHash;
        verifierBackendHash = TRON_GROTH16_BACKEND_HASH;
        proofFamilyHash = configuredProofFamilyHash;
        networkId = configuredNetworkId;
        expectedSourceDomain = configuredSourceDomain;
        expectedTargetDomain = configuredTargetDomain;

        emit VerifierBound(
            address(this),
            expectedVerifierKeyHash,
            TRON_GROTH16_BACKEND_HASH,
            configuredProofFamilyHash
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
        uint256 proofVersion;
        bytes32 proofMessageId;
        uint32 proofSourceDomain;
        bytes32 proofCommitmentRoot;
        (
            proofVersion,
            proofMessageId,
            proofSourceDomain,
            proofCommitmentRoot
        ) = _proofHeader(proofBytes);
        require(
            proofVersion == GROTH16_PROOF_VERSION,
            "Unsupported Groth16 proof version"
        );
        require(
            proofMessageId == publicInputs[0],
            "Proof message id mismatch"
        );
        require(
            proofSourceDomain == expectedSourceDomain,
            "Unexpected source domain"
        );
        require(
            proofCommitmentRoot == publicInputs[3],
            "Proof commitment root mismatch"
        );
        require(publicInputs[3] != bytes32(0), "Commitment root is required");
        require(publicInputs[4] != bytes32(0), "Finality height is required");
        require(publicInputs[5] != bytes32(0), "Finality block hash is required");

        bytes32 bindingHash = destinationBindingHash();
        uint32 sourceDomain;
        bytes32 commitmentRoot;
        (messageId, sourceDomain, commitmentRoot) = this.verifySccpMessageProof(
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
        require(!usedMessageProofs[messageId], "Message proof already used");

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

    function emitDestinationBindingConfigured() external returns (bytes32) {
        bytes32 codeHash = verifierCodeHash();
        bytes32 bindingHash = destinationBindingHash();
        emit DestinationBindingConfigured(
            bindingHash,
            codeHash,
            verifierKeyHash,
            networkId,
            expectedSourceDomain,
            expectedTargetDomain
        );
        return bindingHash;
    }

    function verifierCodeHash() public view returns (bytes32) {
        return _runtimeCodeHash();
    }

    function destinationBindingHash() public view returns (bytes32) {
        return _destinationBindingHashFor(
            verifierCodeHash(),
            verifierKeyHash,
            verifierBackendHash,
            proofFamilyHash,
            networkId,
            expectedSourceDomain,
            expectedTargetDomain
        );
    }

    function _runtimeCodeHash() internal view returns (bytes32 codeHash) {
        address account = address(this);
        assembly {
            codeHash := extcodehash(account)
        }
    }

    function _destinationBindingHashFor(
        bytes32 codeHash,
        bytes32 keyHash,
        bytes32 backendHash,
        bytes32 familyHash,
        bytes32 configuredNetworkId,
        uint32 configuredSourceDomain,
        uint32 configuredTargetDomain
    )
        internal
        view
        returns (bytes32)
    {
        return keccak256(
            abi.encode(
                DESTINATION_BINDING_DOMAIN_SEPARATOR,
                backendHash,
                familyHash,
                configuredNetworkId,
                uint256(configuredSourceDomain),
                uint256(configuredTargetDomain),
                _tronAddressWord(address(this)),
                codeHash,
                keyHash
            )
        );
    }

    function _abiWordU32(uint32 value) internal pure returns (bytes32 out) {
        out = bytes32(uint256(value));
    }

    function _proofHeader(bytes calldata proofBytes)
        internal
        pure
        returns (
            uint256 rawVersion,
            bytes32 rawMessageId,
            uint32 sourceDomain,
            bytes32 rawCommitmentRoot
        )
    {
        uint256 rawSourceDomain;
        uint256[2] memory rawA;
        uint256[4] memory rawB;
        uint256[2] memory rawC;
        require(
            proofBytes.length == GROTH16_PROOF_ABI_BYTE_LENGTH,
            "Unexpected Groth16 proof length"
        );
        (
            rawVersion,
            rawMessageId,
            rawSourceDomain,
            rawCommitmentRoot,
            rawA,
            rawB,
            rawC
        ) = abi.decode(
            proofBytes,
            (uint256, bytes32, uint256, bytes32, uint256[2], uint256[4], uint256[2])
        );
        rawA;
        rawB;
        rawC;
        require(rawSourceDomain <= type(uint32).max, "Source domain overflow");
        sourceDomain = uint32(rawSourceDomain);
    }

    function _tronAddressWord(address account) internal pure returns (bytes32) {
        return bytes32((uint256(0x41) << 160) | uint256(uint160(account)));
    }
}

// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

import "./ISccpMessageVerifier.sol";

/**
 * @title SccpSecp256k1MessageVerifier
 * @dev Production SCCP verifier for EVM lanes.
 *
 * The native SCCP proof is verified off-chain and reduced to a secp256k1
 * attestation over `(message_id, source_domain, commitment_root,
 * public_inputs_hash, statement_hash, native_proof_hash,
 * destination_binding_hash)`. On-chain we only verify that enough authorized
 * attestors signed that digest using EVM-native `keccak256` and `ecrecover`.
 */
contract SccpSecp256k1MessageVerifier is ISccpMessageVerifier {
    uint256 private constant ATTESTATION_VERSION = 1;
    bytes32 private constant ATTESTATION_DOMAIN_SEPARATOR =
        keccak256("iroha:sccp:evm-attestation:v1");
    uint256 private constant SECP256K1_HALF_ORDER =
        0x7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0;

    mapping(address => bool) public authorizedSigners;
    uint256 public authorizedSignerCount;
    uint256 public minimumSignatures;

    event SignerConfigured(address indexed signer, bool authorized);
    event MinimumSignaturesConfigured(uint256 minimumSignatures);

    constructor(address[] memory initialSigners, uint256 initialMinimumSignatures) {
        _configureSigners(initialSigners, true);
        _setMinimumSignatures(initialMinimumSignatures);
    }

    function verifySccpMessageProof(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes32 destinationBindingHash
    )
        external
        view
        override
        returns (
            bytes32 messageId,
            uint32 sourceDomain,
            bytes32 commitmentRoot
        )
    {
        uint256 rawVersion;
        uint256 rawSourceDomain;
        bytes32 nativeProofHash;
        bytes32 attestedDestinationBindingHash;
        bytes memory signatures;

        (
            rawVersion,
            messageId,
            rawSourceDomain,
            commitmentRoot,
            nativeProofHash,
            attestedDestinationBindingHash,
            signatures
        ) = abi.decode(
            proofBytes,
            (uint256, bytes32, uint256, bytes32, bytes32, bytes32, bytes)
        );

        require(rawVersion == ATTESTATION_VERSION, "Unsupported attestation version");
        require(messageId != bytes32(0), "Message id is required");
        require(rawSourceDomain <= type(uint32).max, "Source domain overflow");
        require(signatures.length % 65 == 0, "Recoverable signatures must be 65 bytes");
        require(
            attestedDestinationBindingHash == destinationBindingHash,
            "Destination binding mismatch"
        );

        bytes32 attestationDigest = _attestationDigest(
            messageId,
            rawSourceDomain,
            commitmentRoot,
            publicInputs,
            statementHash,
            nativeProofHash,
            attestedDestinationBindingHash
        );

        uint256 signatureCount = signatures.length / 65;
        require(signatureCount >= minimumSignatures, "Not enough signatures");

        address[] memory seen = new address[](signatureCount);
        uint256 validSignatures = 0;

        for (uint256 i = 0; i < signatureCount; i++) {
            address signer = _recoverSigner(attestationDigest, signatures, i);
            require(authorizedSigners[signer], "Signer is not authorized");

            for (uint256 j = 0; j < validSignatures; j++) {
                require(seen[j] != signer, "Duplicate signer");
            }

            seen[validSignatures] = signer;
            validSignatures += 1;
        }

        require(validSignatures >= minimumSignatures, "Signer quorum not met");
        sourceDomain = uint32(rawSourceDomain);
    }

    function _attestationDigest(
        bytes32 messageId,
        uint256 sourceDomain,
        bytes32 commitmentRoot,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes32 nativeProofHash,
        bytes32 destinationBindingHash
    ) internal pure returns (bytes32) {
        return keccak256(
            abi.encode(
                ATTESTATION_DOMAIN_SEPARATOR,
                messageId,
                sourceDomain,
                commitmentRoot,
                _publicInputsHash(publicInputs),
                statementHash,
                nativeProofHash,
                destinationBindingHash
            )
        );
    }

    function _publicInputsHash(bytes32[6] calldata publicInputs)
        internal
        pure
        returns (bytes32)
    {
        return keccak256(
            abi.encode(
                publicInputs[0],
                publicInputs[1],
                publicInputs[2],
                publicInputs[3],
                publicInputs[4],
                publicInputs[5]
            )
        );
    }

    function _configureSigners(address[] memory signers, bool authorized) internal {
        for (uint256 i = 0; i < signers.length; i++) {
            address signer = signers[i];
            require(signer != address(0), "Signer address is required");
            if (authorizedSigners[signer] == authorized) {
                continue;
            }
            authorizedSigners[signer] = authorized;
            if (authorized) {
                authorizedSignerCount += 1;
            } else {
                authorizedSignerCount -= 1;
            }
            emit SignerConfigured(signer, authorized);
        }
    }

    function _setMinimumSignatures(uint256 newMinimumSignatures) internal {
        require(newMinimumSignatures > 0, "Minimum signatures must be positive");
        require(
            newMinimumSignatures <= authorizedSignerCount,
            "Minimum signatures exceed signer count"
        );
        minimumSignatures = newMinimumSignatures;
        emit MinimumSignaturesConfigured(newMinimumSignatures);
    }

    function _recoverSigner(
        bytes32 digest,
        bytes memory signatures,
        uint256 index
    ) internal pure returns (address signer) {
        uint256 offset = index * 65;
        bytes32 r;
        bytes32 s;
        uint8 v;

        assembly {
            let ptr := add(add(signatures, 32), offset)
            r := mload(ptr)
            s := mload(add(ptr, 32))
            v := byte(0, mload(add(ptr, 64)))
        }

        require(v == 27 || v == 28, "Invalid recovery id");
        require(uint256(s) <= SECP256K1_HALF_ORDER, "High-S signatures are rejected");

        signer = ecrecover(digest, v, r, s);
        require(signer != address(0), "Signature recovery failed");
    }
}

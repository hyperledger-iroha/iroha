// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

import "./ISccpMessageVerifier.sol";

/**
 * @title SccpSecp256k1MessageVerifier
 * @dev Reference-only SCCP verifier for EVM lanes.
 *
 * This contract verifies only a secp256k1 attestation over
 * `(message_id, source_domain, commitment_root, public_inputs_hash,
 * statement_hash, native_proof_hash, destination_binding_hash)`. It does not
 * verify the native SCCP proof and must not be used as a production verifier.
 */
contract SccpSecp256k1MessageVerifier is ISccpMessageVerifier {
    uint256 private constant ATTESTATION_VERSION = 1;
    uint256 private constant ATTESTATION_ABI_HEAD_LENGTH = 7 * 32;
    bytes32 private constant ATTESTATION_DOMAIN_SEPARATOR =
        keccak256("iroha:sccp:evm-attestation:v1");
    uint256 private constant SECP256K1_HALF_ORDER =
        0x7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0;

    mapping(address => bool) public authorizedSigners;
    uint256 public authorizedSignerCount;
    uint256 public minimumSignatures;

    struct DecodedAttestation {
        uint256 version;
        bytes32 messageId;
        uint256 sourceDomain;
        bytes32 commitmentRoot;
        bytes32 nativeProofHash;
        bytes32 destinationBindingHash;
        bytes signatures;
    }

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
        DecodedAttestation memory attestation;
        (
            attestation.version,
            attestation.messageId,
            attestation.sourceDomain,
            attestation.commitmentRoot,
            attestation.nativeProofHash,
            attestation.destinationBindingHash,
            attestation.signatures
        ) = abi.decode(
            proofBytes,
            (uint256, bytes32, uint256, bytes32, bytes32, bytes32, bytes)
        );

        _validateAttestation(
            proofBytes,
            publicInputs,
            statementHash,
            destinationBindingHash,
            attestation
        );

        bytes32 attestationDigest = _attestationDigest(
            attestation.messageId,
            attestation.sourceDomain,
            attestation.commitmentRoot,
            publicInputs,
            statementHash,
            attestation.nativeProofHash,
            attestation.destinationBindingHash
        );

        uint256 signatureCount = attestation.signatures.length / 65;
        require(signatureCount >= minimumSignatures, "Not enough signatures");

        address[] memory seen = new address[](signatureCount);
        uint256 validSignatures = 0;

        for (uint256 i = 0; i < signatureCount; i++) {
            address signer = _recoverSigner(attestationDigest, attestation.signatures, i);
            require(authorizedSigners[signer], "Signer is not authorized");

            for (uint256 j = 0; j < validSignatures; j++) {
                require(seen[j] != signer, "Duplicate signer");
            }

            seen[validSignatures] = signer;
            validSignatures += 1;
        }

        require(validSignatures >= minimumSignatures, "Signer quorum not met");
        messageId = attestation.messageId;
        sourceDomain = uint32(attestation.sourceDomain);
        commitmentRoot = attestation.commitmentRoot;
    }

    function _validateAttestation(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes32 destinationBindingHash,
        DecodedAttestation memory attestation
    ) private pure {
        require(attestation.version == ATTESTATION_VERSION, "Unsupported attestation version");
        require(attestation.messageId != bytes32(0), "Message id is required");
        require(attestation.sourceDomain <= type(uint32).max, "Source domain overflow");
        require(
            _encodedSignaturesOffset(proofBytes) == ATTESTATION_ABI_HEAD_LENGTH,
            "Invalid signatures offset"
        );
        require(
            attestation.signatures.length % 65 == 0,
            "Recoverable signatures must be 65 bytes"
        );
        require(
            proofBytes.length ==
                ATTESTATION_ABI_HEAD_LENGTH + 32 + _paddedLength(attestation.signatures.length),
            "Unexpected attestation length"
        );
        require(
            publicInputs[0] == attestation.messageId,
            "Public input message id mismatch"
        );
        require(publicInputs[1] != bytes32(0), "Payload hash is required");
        uint256 targetDomain = uint256(publicInputs[2]);
        require(targetDomain != 0, "Target domain is required");
        require(targetDomain <= type(uint32).max, "Target domain overflow");
        require(
            targetDomain != attestation.sourceDomain,
            "Source and target domains must differ"
        );
        require(
            publicInputs[3] == attestation.commitmentRoot,
            "Public input commitment root mismatch"
        );
        require(attestation.commitmentRoot != bytes32(0), "Commitment root is required");
        require(publicInputs[4] != bytes32(0), "Finality height is required");
        require(publicInputs[5] != bytes32(0), "Finality block hash is required");
        require(statementHash != bytes32(0), "Statement hash is required");
        require(attestation.nativeProofHash != bytes32(0), "Native proof hash is required");
        require(destinationBindingHash != bytes32(0), "Destination binding hash is required");
        require(
            attestation.destinationBindingHash == destinationBindingHash,
            "Destination binding mismatch"
        );
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

    function _encodedSignaturesOffset(bytes memory proofBytes)
        private
        pure
        returns (uint256 offset)
    {
        assembly {
            offset := mload(add(proofBytes, 0xe0))
        }
    }

    function _paddedLength(uint256 length) private pure returns (uint256) {
        return ((length + 31) / 32) * 32;
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

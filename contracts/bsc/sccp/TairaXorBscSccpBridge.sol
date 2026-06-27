// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

import "../../evm/sccp/ISccpMessageVerifier.sol";

interface ITairaXorToken {
    function mint(address to, uint256 value) external returns (bool);
    function burnFrom(address from, uint256 value) external returns (bool);
}

interface ISccpBscSourceBridge {
    function submitSccpSourceEvent(
        uint32 eventSourceDomain,
        uint32 eventTargetDomain,
        bytes32 sourceEventDigest
    ) external returns (bytes32);
}

/**
 * @title TairaXorBscSccpBridge
 * @dev Route-bound SCCP bridge for XOR between TAIRA and BSC.
 *
 * TAIRA-origin proofs are checked against the production EVM verifier using a
 * route-bridge destination binding, then the bridged token is minted.
 * BSC-origin exits burn the bridged token and emit the SCCP source digest
 * through `SccpBscSourceBridge`.
 */
contract TairaXorBscSccpBridge {
    uint32 private constant SCCP_DOMAIN_SORA = 0;
    uint32 private constant SCCP_DOMAIN_BSC = 2;
    bytes32 private constant BSC_MAINNET_NETWORK_ID = bytes32(uint256(56));
    bytes32 private constant BSC_TESTNET_NETWORK_ID = bytes32(uint256(97));
    bytes32 private constant DESTINATION_BINDING_DOMAIN_SEPARATOR =
        keccak256("iroha:sccp:evm-destination-binding:v1");
    bytes32 private constant PRODUCTION_GROTH16_BACKEND_HASH =
        keccak256("evm-groth16-bn254-v1");
    bytes32 private constant STARK_FRI_PROOF_FAMILY_HASH =
        keccak256("stark-fri-v1");
    bytes32 private constant EMPTY_ACCOUNT_CODE_HASH = keccak256("");
    uint8 private constant SCCP_CODEC_TEXT_UTF8 = 1;
    uint8 private constant SCCP_CODEC_EVM_HEX = 2;
    string private constant SCCP_MSG_PREFIX_TRANSFER_V1 = "sccp:transfer:v1";
    uint256 private constant MAX_TAIRA_RECIPIENT_BYTES = 256;
    bytes32 private constant TAIRA_XOR_BURN_SOURCE_EVENT_PREFIX =
        keccak256("iroha:sccp:taira-xor:burn-source-event:v1");
    uint256 private constant TAIRA_TO_TOKEN_SCALE = 1000000000;

    ITairaXorToken public token;
    ISccpMessageVerifier public verifier;
    ISccpBscSourceBridge public sourceBridge;
    bytes32 public verifierCodeHash;
    bytes32 public verifierKeyHash;
    bytes32 public verifierBackendHash;
    bytes32 public proofFamilyHash;
    bytes32 public routeIdHash;
    bytes32 public assetKeyHash;
    bytes32 public networkId;
    uint32 public expectedSourceDomain;
    uint32 public expectedTargetDomain;
    bytes32 public destinationBindingHash;

    mapping(bytes32 => bool) public usedMessageProofs;
    uint256 public burnNonce;

    event VerifierBound(
        address indexed verifier,
        bytes32 verifierCodeHash,
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

    event TairaXorMintFinalized(
        bytes32 indexed messageId,
        address indexed recipient,
        uint256 amount,
        bytes32 routeIdHash,
        bytes32 assetKeyHash,
        bytes32 payloadHash
    );

    event TairaXorBurnStarted(
        bytes32 indexed sourceEventDigest,
        address indexed burner,
        bytes32 indexed tairaRecipientHash,
        uint256 amount,
        uint256 nonce,
        bytes32 routeIdHash,
        bytes32 assetKeyHash,
        bytes tairaRecipient
    );

    constructor(
        address tokenAddress,
        address verifierAddress,
        address sourceBridgeAddress,
        bytes32 expectedVerifierCodeHash,
        bytes32 expectedVerifierKeyHash,
        string memory verifierBackendKey,
        string memory proofFamily,
        bytes32 configuredNetworkId,
        uint32 configuredSourceDomain,
        uint32 configuredTargetDomain,
        bytes32 configuredRouteIdHash,
        bytes32 configuredAssetKeyHash
    ) {
        require(tokenAddress != address(0), "Token address is required");
        require(verifierAddress != address(0), "Verifier address is required");
        require(sourceBridgeAddress != address(0), "Source bridge address is required");
        require(
            tokenAddress != verifierAddress
                && tokenAddress != sourceBridgeAddress
                && verifierAddress != sourceBridgeAddress,
            "Bridge addresses must differ"
        );
        require(configuredRouteIdHash != bytes32(0), "Route id hash is required");
        require(configuredAssetKeyHash != bytes32(0), "Asset key hash is required");

        token = ITairaXorToken(tokenAddress);
        sourceBridge = ISccpBscSourceBridge(sourceBridgeAddress);
        routeIdHash = configuredRouteIdHash;
        assetKeyHash = configuredAssetKeyHash;
        _configureVerifierBinding(
            verifierAddress,
            expectedVerifierCodeHash,
            expectedVerifierKeyHash,
            verifierBackendKey,
            proofFamily,
            configuredNetworkId,
            configuredSourceDomain,
            configuredTargetDomain
        );
    }

    function _configureVerifierBinding(
        address verifierAddress,
        bytes32 expectedVerifierCodeHash,
        bytes32 expectedVerifierKeyHash,
        string memory verifierBackendKey,
        string memory proofFamily,
        bytes32 configuredNetworkId,
        uint32 configuredSourceDomain,
        uint32 configuredTargetDomain
    ) private {
        _requireVerifierConfig(
            verifierBackendKey,
            proofFamily,
            configuredNetworkId,
            configuredSourceDomain,
            configuredTargetDomain
        );
        bytes32 backendHash = keccak256(bytes(verifierBackendKey));
        bytes32 familyHash = keccak256(bytes(proofFamily));
        bytes32 actualVerifierCodeHash = _codeHash(verifierAddress);
        _requireVerifierIdentity(
            verifierAddress,
            expectedVerifierCodeHash,
            expectedVerifierKeyHash,
            actualVerifierCodeHash
        );

        verifier = ISccpMessageVerifier(verifierAddress);
        verifierCodeHash = actualVerifierCodeHash;
        verifierKeyHash = expectedVerifierKeyHash;
        verifierBackendHash = backendHash;
        proofFamilyHash = familyHash;
        networkId = configuredNetworkId;
        expectedSourceDomain = configuredSourceDomain;
        expectedTargetDomain = configuredTargetDomain;
        bytes32 configuredDestinationBindingHash =
            _currentDestinationBindingHash(verifierAddress);
        destinationBindingHash = configuredDestinationBindingHash;

        emit VerifierBound(
            verifierAddress,
            actualVerifierCodeHash,
            expectedVerifierKeyHash,
            backendHash,
            familyHash
        );
        emit DestinationBindingConfigured(
            configuredDestinationBindingHash,
            actualVerifierCodeHash,
            expectedVerifierKeyHash,
            configuredNetworkId,
            configuredSourceDomain,
            configuredTargetDomain
        );
    }

    function _requireVerifierConfig(
        string memory verifierBackendKey,
        string memory proofFamily,
        bytes32 configuredNetworkId,
        uint32 configuredSourceDomain,
        uint32 configuredTargetDomain
    ) private pure {
        require(
            bytes(verifierBackendKey).length != 0,
            "Verifier backend key is required"
        );
        require(bytes(proofFamily).length != 0, "Proof family is required");
        require(configuredNetworkId != bytes32(0), "Network id is required");
        require(
            configuredNetworkId == BSC_MAINNET_NETWORK_ID ||
                configuredNetworkId == BSC_TESTNET_NETWORK_ID,
            "Network id must be BSC mainnet or testnet"
        );
        require(
            configuredSourceDomain == SCCP_DOMAIN_SORA,
            "Source domain must be SORA"
        );
        require(
            configuredTargetDomain == SCCP_DOMAIN_BSC,
            "Target domain must be BSC"
        );
        require(
            configuredSourceDomain != configuredTargetDomain,
            "Source and target domains must differ"
        );
        require(
            keccak256(bytes(verifierBackendKey)) == PRODUCTION_GROTH16_BACKEND_HASH,
            "Unsupported verifier backend"
        );
        require(
            keccak256(bytes(proofFamily)) == STARK_FRI_PROOF_FAMILY_HASH,
            "Proof family must be stark-fri-v1"
        );
    }

    function _requireVerifierIdentity(
        address verifierAddress,
        bytes32 expectedVerifierCodeHash,
        bytes32 expectedVerifierKeyHash,
        bytes32 actualVerifierCodeHash
    ) private view {
        require(
            expectedVerifierCodeHash != bytes32(0),
            "Verifier code hash is required"
        );
        require(
            actualVerifierCodeHash != bytes32(0)
                && actualVerifierCodeHash != EMPTY_ACCOUNT_CODE_HASH,
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
    }

    function finalizeFromTaira(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes calldata canonicalPayloadBytes
    ) external returns (bytes32 messageId) {
        require(statementHash != bytes32(0), "Statement hash is required");
        require(
            publicInputs[2] == _abiWordU32(SCCP_DOMAIN_BSC),
            "Unexpected target domain"
        );
        (address recipient, uint256 tairaAmount) =
            _parseTairaXorTransferPayload(canonicalPayloadBytes);
        uint256 tokenAmount = _scaleTairaAmount(tairaAmount);
        bytes32 expectedMessageId =
            tairaXorTransferMessageId(canonicalPayloadBytes);
        require(publicInputs[0] == expectedMessageId, "Message id mismatch");
        bytes32 payloadHash = publicInputs[1];
        require(payloadHash != bytes32(0), "Payload hash is required");

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
        require(sourceDomain == SCCP_DOMAIN_SORA, "Unexpected source domain");
        require(publicInputs[0] == messageId, "Public inputs message id mismatch");
        require(
            publicInputs[3] == commitmentRoot,
            "Public inputs commitment root mismatch"
        );
        require(!usedMessageProofs[messageId], "Message proof already used");

        usedMessageProofs[messageId] = true;
        require(token.mint(recipient, tokenAmount), "Token mint failed");

        emit TairaXorMintFinalized(
            messageId,
            recipient,
            tokenAmount,
            routeIdHash,
            assetKeyHash,
            payloadHash
        );
    }

    function burnToTaira(
        bytes32 submittedRouteIdHash,
        bytes32 submittedAssetKeyHash,
        bytes calldata tairaRecipient,
        uint256 tokenAmount
    ) external returns (bytes32 sourceEventDigest) {
        require(submittedRouteIdHash == routeIdHash, "Unexpected route");
        require(submittedAssetKeyHash == assetKeyHash, "Unexpected asset");
        require(tairaRecipient.length != 0, "TAIRA recipient is required");
        require(
            tairaRecipient.length <= MAX_TAIRA_RECIPIENT_BYTES,
            "TAIRA recipient is too long"
        );
        require(tokenAmount != 0, "Amount is required");
        require(
            tokenAmount % TAIRA_TO_TOKEN_SCALE == 0,
            "Amount must align to TAIRA scale"
        );
        require(burnNonce != uint256(-1), "Burn nonce exhausted");

        uint256 nonce = burnNonce;
        burnNonce = nonce + 1;
        uint256 tairaAmount = tokenAmount / TAIRA_TO_TOKEN_SCALE;
        bytes32 recipientHash = keccak256(tairaRecipient);
        sourceEventDigest = tairaXorBurnSourceEventDigest(
            submittedRouteIdHash,
            submittedAssetKeyHash,
            msg.sender,
            recipientHash,
            tairaAmount,
            nonce
        );

        require(token.burnFrom(msg.sender, tokenAmount), "Token burn failed");
        require(
            sourceBridge.submitSccpSourceEvent(
                SCCP_DOMAIN_BSC,
                SCCP_DOMAIN_SORA,
                sourceEventDigest
            ) == sourceEventDigest,
            "Source bridge submit failed"
        );

        emit TairaXorBurnStarted(
            sourceEventDigest,
            msg.sender,
            recipientHash,
            tairaAmount,
            nonce,
            submittedRouteIdHash,
            submittedAssetKeyHash,
            tairaRecipient
        );
    }

    function _scaleTairaAmount(uint256 amount) private pure returns (uint256) {
        require(
            amount <= uint256(-1) / TAIRA_TO_TOKEN_SCALE,
            "Amount exceeds token scale"
        );
        return amount * TAIRA_TO_TOKEN_SCALE;
    }

    function tairaXorTransferMessageId(
        bytes calldata canonicalPayloadBytes
    ) public pure returns (bytes32) {
        return keccak256(
            abi.encodePacked(SCCP_MSG_PREFIX_TRANSFER_V1, canonicalPayloadBytes)
        );
    }

    function tairaXorBurnSourceEventDigest(
        bytes32 submittedRouteIdHash,
        bytes32 submittedAssetKeyHash,
        address burner,
        bytes32 tairaRecipientHash,
        uint256 amount,
        uint256 nonce
    ) public view returns (bytes32) {
        return keccak256(
            abi.encode(
                TAIRA_XOR_BURN_SOURCE_EVENT_PREFIX,
                submittedRouteIdHash,
                submittedAssetKeyHash,
                address(this),
                burner,
                tairaRecipientHash,
                amount,
                nonce
            )
        );
    }

    function _abiWordU32(uint32 value) private pure returns (bytes32 out) {
        out = bytes32(uint256(value));
    }

    function _currentDestinationBindingHash(
        address verifierAddress
    ) private view returns (bytes32) {
        return keccak256(
            abi.encode(
                DESTINATION_BINDING_DOMAIN_SEPARATOR,
                verifierBackendHash,
                proofFamilyHash,
                networkId,
                uint256(expectedSourceDomain),
                uint256(expectedTargetDomain),
                verifierAddress,
                address(this),
                verifierCodeHash,
                verifierKeyHash
            )
        );
    }

    function _codeHash(address account) private view returns (bytes32 codeHash) {
        assembly {
            codeHash := extcodehash(account)
        }
    }

    function _verifyingKeyHash(
        address verifierAddress
    ) private view returns (bytes32 keyHash) {
        (bool success, bytes memory data) = verifierAddress.staticcall(
            abi.encodeWithSignature("verifyingKeyHash()")
        );
        require(success && data.length == 32, "Verifier key hash unavailable");
        keyHash = abi.decode(data, (bytes32));
    }

    function _parseTairaXorTransferPayload(
        bytes calldata payload
    ) private pure returns (address recipient, uint256 amount) {
        uint256 offset = 0;
        require(_readU8(payload, offset) == 1, "Unsupported payload version");
        offset += 1;
        require(
            _readU32Le(payload, offset) == SCCP_DOMAIN_SORA,
            "Unexpected source domain"
        );
        offset += 4;
        require(
            _readU32Le(payload, offset) == SCCP_DOMAIN_BSC,
            "Unexpected destination domain"
        );
        offset += 4;
        offset += 8; // nonce
        require(
            _readU32Le(payload, offset) == SCCP_DOMAIN_SORA,
            "Unexpected asset home domain"
        );
        offset += 4;

        require(
            _readU8(payload, offset) == SCCP_CODEC_TEXT_UTF8,
            "Unexpected asset codec"
        );
        offset += 1;
        uint256 valueOffset;
        uint256 valueLength;
        (valueOffset, valueLength, offset) = _readVecRange(payload, offset);
        require(_isTairaXorAssetKey(payload, valueOffset, valueLength), "Unexpected asset");

        amount = _readU128Le(payload, offset);
        require(amount != 0, "Amount is required");
        offset += 16;

        require(
            _readU8(payload, offset) == SCCP_CODEC_TEXT_UTF8,
            "Unexpected sender codec"
        );
        offset += 1;
        (valueOffset, valueLength, offset) = _readVecRange(payload, offset);
        require(valueLength != 0, "Sender is required");

        require(
            _readU8(payload, offset) == SCCP_CODEC_EVM_HEX,
            "Unexpected recipient codec"
        );
        offset += 1;
        (valueOffset, valueLength, offset) = _readVecRange(payload, offset);
        recipient = _decodeEvmAddress(payload, valueOffset, valueLength);

        require(
            _readU8(payload, offset) == SCCP_CODEC_TEXT_UTF8,
            "Unexpected route codec"
        );
        offset += 1;
        (valueOffset, valueLength, offset) = _readVecRange(payload, offset);
        require(_isTairaBscXorRoute(payload, valueOffset, valueLength), "Unexpected route");
        require(offset == payload.length, "Trailing payload bytes");
    }

    function _readU8(
        bytes calldata payload,
        uint256 offset
    ) private pure returns (uint8) {
        require(offset < payload.length, "Payload is too short");
        return uint8(payload[offset]);
    }

    function _readU32Le(
        bytes calldata payload,
        uint256 offset
    ) private pure returns (uint32) {
        require(
            offset <= payload.length && payload.length - offset >= 4,
            "Payload is too short"
        );
        return uint32(uint8(payload[offset]))
            | (uint32(uint8(payload[offset + 1])) << 8)
            | (uint32(uint8(payload[offset + 2])) << 16)
            | (uint32(uint8(payload[offset + 3])) << 24);
    }

    function _readU128Le(
        bytes calldata payload,
        uint256 offset
    ) private pure returns (uint256 value) {
        require(
            offset <= payload.length && payload.length - offset >= 16,
            "Payload is too short"
        );
        for (uint256 i = 0; i < 16; i++) {
            value |= uint256(uint8(payload[offset + i])) << (i * 8);
        }
    }

    function _readVecRange(
        bytes calldata payload,
        uint256 offset
    )
        private
        pure
        returns (
            uint256 valueOffset,
            uint256 valueLength,
            uint256 nextOffset
        )
    {
        valueLength = uint256(_readU32Le(payload, offset));
        valueOffset = offset + 4;
        require(
            valueOffset <= payload.length
                && payload.length - valueOffset >= valueLength,
            "Payload is too short"
        );
        nextOffset = valueOffset + valueLength;
    }

    function _isTairaXorAssetKey(
        bytes calldata payload,
        uint256 offset,
        uint256 length
    ) private pure returns (bool) {
        return length == 3
            && payload[offset] == bytes1(0x78)
            && payload[offset + 1] == bytes1(0x6f)
            && payload[offset + 2] == bytes1(0x72);
    }

    function _isTairaBscXorRoute(
        bytes calldata payload,
        uint256 offset,
        uint256 length
    ) private pure returns (bool) {
        return length == 13
            && payload[offset] == bytes1(0x74)
            && payload[offset + 1] == bytes1(0x61)
            && payload[offset + 2] == bytes1(0x69)
            && payload[offset + 3] == bytes1(0x72)
            && payload[offset + 4] == bytes1(0x61)
            && payload[offset + 5] == bytes1(0x5f)
            && payload[offset + 6] == bytes1(0x62)
            && payload[offset + 7] == bytes1(0x73)
            && payload[offset + 8] == bytes1(0x63)
            && payload[offset + 9] == bytes1(0x5f)
            && payload[offset + 10] == bytes1(0x78)
            && payload[offset + 11] == bytes1(0x6f)
            && payload[offset + 12] == bytes1(0x72);
    }

    function _decodeEvmAddress(
        bytes calldata payload,
        uint256 offset,
        uint256 length
    ) private pure returns (address) {
        require(length == 42, "Recipient must be EIP-55 EVM address");
        require(
            payload[offset] == bytes1(0x30) && payload[offset + 1] == bytes1(0x78),
            "Recipient must be 0x-prefixed EVM address"
        );
        bytes memory lowercasePayload = new bytes(40);
        for (uint256 i = 0; i < 40; i++) {
            lowercasePayload[i] = bytes1(_lowercaseHexChar(uint8(payload[offset + 2 + i])));
        }
        bytes32 checksum = keccak256(lowercasePayload);
        uint160 rawAddress = 0;
        bool nonZero = false;
        for (uint256 i = 0; i < 40; i++) {
            uint8 char = uint8(payload[offset + 2 + i]);
            uint8 nibble = _hexNibble(char);
            if (_isHexAlpha(char)) {
                uint8 checksumByte = uint8(checksum[i / 2]);
                uint8 checksumNibble =
                    i % 2 == 0 ? checksumByte >> 4 : checksumByte & 0x0f;
                bool shouldBeUppercase = checksumNibble >= 8;
                require(
                    shouldBeUppercase == _isUpperHexAlpha(char),
                    "Recipient must be canonical EIP-55"
                );
            }
            rawAddress = (rawAddress << 4) | uint160(nibble);
            if (nibble != 0) {
                nonZero = true;
            }
        }
        require(nonZero, "Recipient address is required");
        return address(rawAddress);
    }

    function _hexNibble(uint8 char) private pure returns (uint8) {
        if (char >= 0x30 && char <= 0x39) {
            return char - 0x30;
        }
        if (char >= 0x41 && char <= 0x46) {
            return char - 0x41 + 10;
        }
        if (char >= 0x61 && char <= 0x66) {
            return char - 0x61 + 10;
        }
        revert("Recipient must contain hex");
    }

    function _lowercaseHexChar(uint8 char) private pure returns (uint8) {
        if (char >= 0x30 && char <= 0x39) {
            return char;
        }
        if (char >= 0x41 && char <= 0x46) {
            return char + 32;
        }
        if (char >= 0x61 && char <= 0x66) {
            return char;
        }
        revert("Recipient must contain hex");
    }

    function _isHexAlpha(uint8 char) private pure returns (bool) {
        return (char >= 0x41 && char <= 0x46) || (char >= 0x61 && char <= 0x66);
    }

    function _isUpperHexAlpha(uint8 char) private pure returns (bool) {
        return char >= 0x41 && char <= 0x46;
    }
}

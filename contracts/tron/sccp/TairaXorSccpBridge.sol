// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

interface ITairaXorToken {
    function mint(address to, uint256 value) external returns (bool);
    function burnFrom(address from, uint256 value) external returns (bool);
}

interface ISccpTronSourceBridge {
    function submitSccpSourceEvent(
        uint32 eventSourceDomain,
        uint32 eventTargetDomain,
        bytes32 sourceEventDigest
    ) external returns (bytes32);
}

interface ISccpTronVerifierView {
    function destinationBindingHash() external view returns (bytes32);
    function networkId() external view returns (bytes32);
    function expectedSourceDomain() external view returns (uint32);
    function expectedTargetDomain() external view returns (uint32);
    function verifySccpMessageProof(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes32 submittedDestinationBindingHash
    )
        external
        view
        returns (
            bytes32 messageId,
            uint32 sourceDomain,
            bytes32 commitmentRoot
        );
}

/**
 * @title TairaXorSccpBridge
 * @dev Route-bound SCCP bridge for XOR between TAIRA and TRON.
 *
 * TAIRA-origin proofs are checked against the production TRON verifier binding,
 * then the bridged token is minted. TRON-origin exits burn the bridged token and
 * emit the SCCP source digest through `SccpTronSourceBridge`.
 */
contract TairaXorSccpBridge {
    uint32 private constant SCCP_DOMAIN_SORA = 0;
    uint32 private constant SCCP_DOMAIN_TRON = 5;
    uint8 private constant SCCP_CODEC_TEXT_UTF8 = 1;
    uint8 private constant SCCP_CODEC_TRON_BASE58CHECK = 5;
    string private constant SCCP_MSG_PREFIX_TRANSFER_V1 = "sccp:transfer:v1";
    uint256 private constant MAX_TAIRA_RECIPIENT_BYTES = 256;
    bytes32 private constant TAIRA_XOR_BURN_SOURCE_EVENT_PREFIX =
        keccak256("iroha:sccp:taira-xor:burn-source-event:v1");

    ITairaXorToken public immutable token;
    ISccpTronVerifierView public immutable verifier;
    ISccpTronSourceBridge public immutable sourceBridge;
    bytes32 public immutable routeIdHash;
    bytes32 public immutable assetKeyHash;
    bytes32 public immutable networkId;
    bytes32 public immutable destinationBindingHash;

    mapping(bytes32 => bool) public usedMessageProofs;
    uint256 public burnNonce;

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

        ISccpTronVerifierView configuredVerifier =
            ISccpTronVerifierView(verifierAddress);
        require(
            configuredVerifier.expectedSourceDomain() == SCCP_DOMAIN_SORA,
            "Verifier source domain must be SORA"
        );
        require(
            configuredVerifier.expectedTargetDomain() == SCCP_DOMAIN_TRON,
            "Verifier target domain must be TRON"
        );
        bytes32 configuredNetworkId = configuredVerifier.networkId();
        bytes32 configuredDestinationBindingHash =
            configuredVerifier.destinationBindingHash();
        require(configuredNetworkId != bytes32(0), "Network id is required");
        require(
            configuredDestinationBindingHash != bytes32(0),
            "Destination binding hash is required"
        );

        token = ITairaXorToken(tokenAddress);
        verifier = configuredVerifier;
        sourceBridge = ISccpTronSourceBridge(sourceBridgeAddress);
        routeIdHash = configuredRouteIdHash;
        assetKeyHash = configuredAssetKeyHash;
        networkId = configuredNetworkId;
        destinationBindingHash = configuredDestinationBindingHash;
    }

    function finalizeFromTaira(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes calldata canonicalPayloadBytes
    ) external returns (bytes32 messageId) {
        require(statementHash != bytes32(0), "Statement hash is required");
        require(
            publicInputs[2] == _abiWordU32(SCCP_DOMAIN_TRON),
            "Unexpected target domain"
        );
        (address recipient, uint256 amount) =
            _parseTairaXorTransferPayload(canonicalPayloadBytes);
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
        require(token.mint(recipient, amount), "Token mint failed");

        emit TairaXorMintFinalized(
            messageId,
            recipient,
            amount,
            routeIdHash,
            assetKeyHash,
            payloadHash
        );
    }

    function burnToTaira(
        bytes32 submittedRouteIdHash,
        bytes32 submittedAssetKeyHash,
        bytes calldata tairaRecipient,
        uint256 amount
    ) external returns (bytes32 sourceEventDigest) {
        require(submittedRouteIdHash == routeIdHash, "Unexpected route");
        require(submittedAssetKeyHash == assetKeyHash, "Unexpected asset");
        require(tairaRecipient.length != 0, "TAIRA recipient is required");
        require(
            tairaRecipient.length <= MAX_TAIRA_RECIPIENT_BYTES,
            "TAIRA recipient is too long"
        );
        require(amount != 0, "Amount is required");
        require(burnNonce != uint256(-1), "Burn nonce exhausted");

        uint256 nonce = burnNonce;
        burnNonce = nonce + 1;
        bytes32 recipientHash = keccak256(tairaRecipient);
        sourceEventDigest = tairaXorBurnSourceEventDigest(
            submittedRouteIdHash,
            submittedAssetKeyHash,
            msg.sender,
            recipientHash,
            amount,
            nonce
        );

        require(token.burnFrom(msg.sender, amount), "Token burn failed");
        require(
            sourceBridge.submitSccpSourceEvent(
                SCCP_DOMAIN_TRON,
                SCCP_DOMAIN_SORA,
                sourceEventDigest
            ) == sourceEventDigest,
            "Source bridge submit failed"
        );

        emit TairaXorBurnStarted(
            sourceEventDigest,
            msg.sender,
            recipientHash,
            amount,
            nonce,
            submittedRouteIdHash,
            submittedAssetKeyHash,
            tairaRecipient
        );
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
            _readU32Le(payload, offset) == SCCP_DOMAIN_TRON,
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
            _readU8(payload, offset) == SCCP_CODEC_TRON_BASE58CHECK,
            "Unexpected recipient codec"
        );
        offset += 1;
        (valueOffset, valueLength, offset) = _readVecRange(payload, offset);
        recipient = _decodeTronBase58CheckAddress(payload, valueOffset, valueLength);

        require(
            _readU8(payload, offset) == SCCP_CODEC_TEXT_UTF8,
            "Unexpected route codec"
        );
        offset += 1;
        (valueOffset, valueLength, offset) = _readVecRange(payload, offset);
        require(_isTairaTronXorRoute(payload, valueOffset, valueLength), "Unexpected route");
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

    function _isTairaTronXorRoute(
        bytes calldata payload,
        uint256 offset,
        uint256 length
    ) private pure returns (bool) {
        return length == 14
            && payload[offset] == bytes1(0x74)
            && payload[offset + 1] == bytes1(0x61)
            && payload[offset + 2] == bytes1(0x69)
            && payload[offset + 3] == bytes1(0x72)
            && payload[offset + 4] == bytes1(0x61)
            && payload[offset + 5] == bytes1(0x5f)
            && payload[offset + 6] == bytes1(0x74)
            && payload[offset + 7] == bytes1(0x72)
            && payload[offset + 8] == bytes1(0x6f)
            && payload[offset + 9] == bytes1(0x6e)
            && payload[offset + 10] == bytes1(0x5f)
            && payload[offset + 11] == bytes1(0x78)
            && payload[offset + 12] == bytes1(0x6f)
            && payload[offset + 13] == bytes1(0x72);
    }

    function _decodeTronBase58CheckAddress(
        bytes calldata payload,
        uint256 offset,
        uint256 length
    ) private pure returns (address) {
        require(length == 34, "Recipient must be TRON address");
        require(payload[offset] == bytes1(0x54), "Recipient must be TRON address");
        uint256 value = 0;
        for (uint256 i = 0; i < length; i++) {
            value = value * 58 + _base58Digit(uint8(payload[offset + i]));
        }

        bytes memory decoded = new bytes(25);
        uint256 remaining = value;
        for (uint256 i = 25; i > 0; i--) {
            decoded[i - 1] = bytes1(uint8(remaining));
            remaining >>= 8;
        }
        require(remaining == 0, "Recipient address overflow");

        bytes memory addressPayload = new bytes(21);
        uint160 rawAddress = 0;
        bool nonZero = false;
        for (uint256 i = 0; i < 21; i++) {
            addressPayload[i] = decoded[i];
            if (i > 0) {
                uint8 byteValue = uint8(decoded[i]);
                rawAddress = (rawAddress << 8) | uint160(byteValue);
                if (byteValue != 0) {
                    nonZero = true;
                }
            }
        }
        require(decoded[0] == bytes1(0x41), "Recipient must use TRON prefix");
        require(nonZero, "Recipient address is required");
        bytes32 expectedChecksum = sha256(abi.encodePacked(sha256(addressPayload)));
        require(
            decoded[21] == expectedChecksum[0]
                && decoded[22] == expectedChecksum[1]
                && decoded[23] == expectedChecksum[2]
                && decoded[24] == expectedChecksum[3],
            "Recipient checksum mismatch"
        );
        return address(rawAddress);
    }

    function _base58Digit(uint8 character) private pure returns (uint8) {
        if (character >= 0x31 && character <= 0x39) {
            return character - 0x31;
        }
        if (character >= 0x41 && character <= 0x48) {
            return character - 0x41 + 9;
        }
        if (character >= 0x4a && character <= 0x4e) {
            return character - 0x4a + 17;
        }
        if (character >= 0x50 && character <= 0x5a) {
            return character - 0x50 + 22;
        }
        if (character >= 0x61 && character <= 0x6b) {
            return character - 0x61 + 33;
        }
        if (character >= 0x6d && character <= 0x7a) {
            return character - 0x6d + 44;
        }
        revert("Recipient must be base58");
    }
}

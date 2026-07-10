// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

import "../../evm/sccp/SccpExactTransferCodec.sol";

interface ITairaXorTronToken {
    function bridge() external view returns (address);
    function mint(address to, uint256 value) external returns (bool);
    function burnFrom(address from, uint256 value) external returns (bool);
}

interface ISccpTronExactVerifier {
    function networkId() external view returns (bytes32);
    function expectedSourceDomain() external view returns (uint32);
    function expectedTargetDomain() external view returns (uint32);
    function verifyingKeyHash() external view returns (bytes32);
    function semanticProofProfileHash() external view returns (bytes32);
    function soraFinalityAnchorHash() external view returns (bytes32);
    function verifySccpMessageProof(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes32 submittedDestinationBindingHash,
        bytes32 submittedRouteConfigurationHash
    ) external view returns (bytes32 messageId, uint32 sourceDomain, bytes32 commitmentRoot);
}

/** Concrete exact-profile XOR transfer route between TRON and SORA Taira. */
contract TairaXorSccpBridge {
    uint32 private constant DOMAIN_SORA = 0;
    uint32 private constant DOMAIN_TRON = 5;
    uint8 private constant CODEC_TEXT = 1;
    uint8 private constant CODEC_TRON21 = 5;
    uint256 private constant MAX_U128 = (uint256(1) << 128) - 1;
    uint256 private constant TAIRA_TO_TOKEN_SCALE = 1000000000;
    bytes private constant ASSET_ID = "xor";
    bytes private constant ROUTE_ID = "taira_tron_xor";
    bytes32 private constant DESTINATION_BINDING_SEPARATOR =
        keccak256("iroha:sccp:tron-destination-binding:v1");
    bytes32 private constant VERIFIER_BACKEND_HASH =
        keccak256("tron-groth16-bn254-v1");
    bytes32 private constant PROOF_FAMILY_HASH = keccak256("stark-fri-v1");
    bytes32 private constant ROUTE_CONFIG_SEPARATOR =
        keccak256("sccp:concrete-route-config:v1");
    bytes32 private constant EMPTY_CODE_HASH = keccak256("");

    ITairaXorTronToken public immutable token;
    ISccpTronExactVerifier public immutable verifier;
    uint8 public immutable tronProfile;
    uint32 public immutable routeRevision;
    bytes32 public immutable tokenCodeHash;
    bytes32 public immutable verifierCodeHash;
    bytes32 public immutable verifierKeyHash;
    bytes32 public immutable semanticProofProfileHash;
    bytes32 public immutable soraFinalityAnchorHash;
    bytes32 public immutable sourceLaneHash;
    bytes32 public immutable destinationLaneHash;
    bytes32 public immutable routeConfigHash;
    bytes32 public immutable destinationBindingHash;

    uint64 public transferNonce;
    mapping(bytes32 => bool) public usedSourceMessages;
    mapping(bytes32 => bool) public usedDestinationMessages;
    uint256 private reentrancyState = 1;

    event SccpTransfer(
        bytes32 indexed laneHash,
        bytes32 indexed messageId,
        bytes32 indexed sourceEventDigest,
        bytes32 payloadHash,
        bytes32 routeConfigHash,
        bytes canonicalPayload
    );
    event TairaXorMintFinalized(
        bytes32 indexed messageId,
        address indexed recipient,
        uint256 amount,
        bytes32 payloadHash
    );

    modifier nonReentrant() {
        require(reentrancyState == 1, "Reentrant bridge call");
        reentrancyState = 2;
        _;
        reentrancyState = 1;
    }

    constructor(
        address tokenAddress,
        address verifierAddress,
        bytes32 expectedVerifierCodeHash,
        bytes32 expectedVerifierKeyHash,
        bytes32 expectedSemanticProofProfileHash,
        bytes32 expectedSoraFinalityAnchorHash,
        uint8 configuredTronProfile,
        uint32 configuredRouteRevision
    ) {
        require(tokenAddress != address(0) && verifierAddress != address(0), "Zero bridge address");
        require(tokenAddress != verifierAddress, "Bridge roles must differ");
        require(configuredTronProfile >= 10 && configuredTronProfile <= 12,
            "Unsupported TRON profile");
        require(configuredRouteRevision != 0, "Route revision is required");
        require(
            expectedSemanticProofProfileHash != bytes32(0),
            "Semantic proof profile hash is required"
        );
        require(
            expectedSoraFinalityAnchorHash != bytes32(0),
            "SORA finality anchor hash is required"
        );
        require(
            expectedSemanticProofProfileHash != expectedSoraFinalityAnchorHash,
            "Semantic profile and finality anchor must differ"
        );
        ITairaXorTronToken configuredToken = ITairaXorTronToken(tokenAddress);
        require(configuredToken.bridge() == address(this), "Token route mismatch");
        ISccpTronExactVerifier configuredVerifier = ISccpTronExactVerifier(verifierAddress);
        require(configuredVerifier.expectedSourceDomain() == DOMAIN_SORA
            && configuredVerifier.expectedTargetDomain() == DOMAIN_TRON,
            "Verifier domain mismatch");
        require(configuredVerifier.networkId() == _networkIdWord(configuredTronProfile),
            "Verifier network mismatch");
        bytes32 actualTokenCodeHash = _codeHash(tokenAddress);
        require(actualTokenCodeHash != bytes32(0) && actualTokenCodeHash != EMPTY_CODE_HASH,
            "Token contract is required");
        bytes32 actualCodeHash = _codeHash(verifierAddress);
        require(expectedVerifierCodeHash != bytes32(0)
            && actualCodeHash == expectedVerifierCodeHash
            && actualCodeHash != EMPTY_CODE_HASH, "Verifier code hash mismatch");
        require(expectedVerifierKeyHash != bytes32(0)
            && configuredVerifier.verifyingKeyHash() == expectedVerifierKeyHash,
            "Verifier key hash mismatch");
        require(
            configuredVerifier.semanticProofProfileHash() == expectedSemanticProofProfileHash,
            "Semantic proof profile hash mismatch"
        );
        require(
            configuredVerifier.soraFinalityAnchorHash() == expectedSoraFinalityAnchorHash,
            "SORA finality anchor hash mismatch"
        );
        bytes32 binding = _routeDestinationBinding(
            configuredTronProfile,
            verifierAddress,
            actualCodeHash,
            expectedVerifierKeyHash,
            expectedSemanticProofProfileHash,
            expectedSoraFinalityAnchorHash
        );

        (bytes32 inboundLaneHash, bytes32 outboundLaneHash) =
            _laneHashes(configuredTronProfile);
        bytes32 deploymentConfigHash = keccak256(abi.encode(
            tokenAddress,
            actualTokenCodeHash,
            verifierAddress,
            actualCodeHash,
            expectedVerifierKeyHash,
            expectedSemanticProofProfileHash,
            expectedSoraFinalityAnchorHash,
            binding
        ));
        bytes32 assetRouteConfigHash = keccak256(abi.encode(
            keccak256(ASSET_ID),
            keccak256(ROUTE_ID),
            configuredRouteRevision,
            TAIRA_TO_TOKEN_SCALE
        ));
        bytes32 exactRouteConfigHash = keccak256(abi.encode(
            ROUTE_CONFIG_SEPARATOR,
            DOMAIN_TRON,
            configuredTronProfile,
            _networkIdWord(configuredTronProfile),
            inboundLaneHash,
            outboundLaneHash,
            deploymentConfigHash,
            assetRouteConfigHash
        ));

        token = configuredToken;
        verifier = configuredVerifier;
        tronProfile = configuredTronProfile;
        routeRevision = configuredRouteRevision;
        tokenCodeHash = actualTokenCodeHash;
        verifierCodeHash = actualCodeHash;
        verifierKeyHash = expectedVerifierKeyHash;
        semanticProofProfileHash = expectedSemanticProofProfileHash;
        soraFinalityAnchorHash = expectedSoraFinalityAnchorHash;
        sourceLaneHash = inboundLaneHash;
        destinationLaneHash = outboundLaneHash;
        routeConfigHash = exactRouteConfigHash;
        destinationBindingHash = binding;
    }

    function _laneHashes(uint8 profile) private pure returns (bytes32 inbound, bytes32 outbound) {
        bytes memory tron = SccpExactTransferCodec.tronNetwork(profile);
        bytes memory taira = SccpExactTransferCodec.tairaNetwork();
        inbound = SccpExactTransferCodec.laneHash(SccpExactTransferCodec.lane(tron, taira));
        outbound = SccpExactTransferCodec.laneHash(SccpExactTransferCodec.lane(taira, tron));
    }

    function _routeDestinationBinding(
        uint8 profile,
        address verifierAddress,
        bytes32 codeHash,
        bytes32 keyHash,
        bytes32 semanticProfileHash,
        bytes32 finalityAnchorHash
    ) private view returns (bytes32) {
        return keccak256(abi.encode(
            DESTINATION_BINDING_SEPARATOR,
            VERIFIER_BACKEND_HASH,
            PROOF_FAMILY_HASH,
            _networkIdWord(profile),
            uint256(DOMAIN_SORA),
            uint256(DOMAIN_TRON),
            _tronAddressWord(verifierAddress),
            _tronAddressWord(address(this)),
            codeHash,
            keyHash,
            semanticProfileHash,
            finalityAnchorHash
        ));
    }

    /** Burn wrapped XOR and emit one exact TRON-to-Taira Transfer statement. */
    function transferToTaira(bytes calldata tairaRecipient, uint256 tokenAmount)
        external nonReentrant returns (bytes32 messageId)
    {
        bytes memory recipient = tairaRecipient;
        require(SccpExactTransferCodec.isCanonicalText(recipient), "Noncanonical Taira recipient");
        require(tokenAmount != 0 && tokenAmount % TAIRA_TO_TOKEN_SCALE == 0,
            "Amount is not aligned to Taira scale");
        uint256 tairaAmount = tokenAmount / TAIRA_TO_TOKEN_SCALE;
        require(tairaAmount != 0 && tairaAmount <= MAX_U128, "Amount exceeds SCCP u128");
        require(transferNonce != type(uint64).max, "Transfer nonce exhausted");
        require(_codeHash(address(token)) == tokenCodeHash, "Token code changed");
        uint64 nonce = transferNonce;
        bytes memory sender = abi.encodePacked(bytes1(0x41), msg.sender);
        SccpExactTransferCodec.TransferFields memory fields;
        fields.sourceDomain = DOMAIN_TRON;
        fields.destinationDomain = DOMAIN_SORA;
        fields.nonce = nonce;
        fields.routeRevision = routeRevision;
        fields.assetHomeDomain = DOMAIN_SORA;
        fields.assetId = ASSET_ID;
        fields.amount = tairaAmount;
        fields.senderCodec = CODEC_TRON21;
        fields.sender = sender;
        fields.recipientCodec = CODEC_TEXT;
        fields.recipient = recipient;
        fields.routeId = ROUTE_ID;
        bytes memory payload = SccpExactTransferCodec.transferPayload(fields);
        bytes memory exactLane = SccpExactTransferCodec.lane(
            SccpExactTransferCodec.tronNetwork(tronProfile),
            SccpExactTransferCodec.tairaNetwork()
        );
        messageId = SccpExactTransferCodec.messageId(exactLane, payload);
        bytes32 canonicalPayloadHash = SccpExactTransferCodec.payloadHash(payload);
        bytes32 eventDigest = SccpExactTransferCodec.sourceEventDigest(
            sourceLaneHash, messageId, canonicalPayloadHash
        );
        require(!usedSourceMessages[messageId], "Source message already used");

        transferNonce = nonce + 1;
        usedSourceMessages[messageId] = true;
        require(token.burnFrom(msg.sender, tokenAmount), "Token burn failed");
        emit SccpTransfer(
            sourceLaneHash,
            messageId,
            eventDigest,
            canonicalPayloadHash,
            routeConfigHash,
            payload
        );
    }

    /** Verify one exact Taira-to-TRON transfer proof and mint wrapped XOR. */
    function finalizeFromTaira(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes calldata canonicalPayloadBytes
    ) external nonReentrant returns (bytes32 messageId) {
        require(statementHash != bytes32(0), "Statement hash is required");
        require(publicInputs[2] == bytes32(uint256(DOMAIN_TRON)), "Unexpected target domain");
        require(_codeHash(address(token)) == tokenCodeHash, "Token code changed");
        bytes memory payload = canonicalPayloadBytes;
        (address recipient, uint256 tairaAmount) = _parseTairaToTronTransfer(payload);
        bytes32 canonicalPayloadHash = SccpExactTransferCodec.payloadHash(payload);
        require(publicInputs[1] == canonicalPayloadHash, "Payload hash mismatch");
        bytes32 expectedMessageId = _destinationMessageId(payload);
        require(publicInputs[0] == expectedMessageId, "Message id mismatch");
        messageId = _verifyDestinationProof(
            proofBytes, publicInputs, statementHash, expectedMessageId
        );
        require(!usedDestinationMessages[messageId], "Destination message already used");
        require(tairaAmount <= uint256(-1) / TAIRA_TO_TOKEN_SCALE, "Token amount overflow");
        uint256 tokenAmount = tairaAmount * TAIRA_TO_TOKEN_SCALE;

        usedDestinationMessages[messageId] = true;
        require(token.mint(recipient, tokenAmount), "Token mint failed");
        emit TairaXorMintFinalized(messageId, recipient, tokenAmount, canonicalPayloadHash);
    }

    /** Derive the exact source-event digest emitted by this route. */
    function sourceEventDigest(bytes32 messageId, bytes32 canonicalPayloadHash)
        external view returns (bytes32)
    {
        return SccpExactTransferCodec.sourceEventDigest(
            sourceLaneHash, messageId, canonicalPayloadHash
        );
    }

    /** Hash one canonical SCCP payload with the exact V1 payload hash. */
    function sccpPayloadHash(bytes calldata canonicalPayload) external pure returns (bytes32) {
        bytes memory payload = canonicalPayload;
        return SccpExactTransferCodec.payloadHash(payload);
    }

    /** Derive the exact Taira-to-TRON message id for one canonical payload. */
    function sccpDestinationMessageId(bytes calldata canonicalPayload)
        external view returns (bytes32)
    {
        bytes memory payload = canonicalPayload;
        return _destinationMessageId(payload);
    }

    function _destinationMessageId(bytes memory payload) private view returns (bytes32) {
        return SccpExactTransferCodec.messageId(
            SccpExactTransferCodec.lane(
                SccpExactTransferCodec.tairaNetwork(),
                SccpExactTransferCodec.tronNetwork(tronProfile)
            ),
            payload
        );
    }

    function _verifyDestinationProof(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes32 expectedMessageId
    ) private view returns (bytes32 messageId) {
        require(_codeHash(address(verifier)) == verifierCodeHash, "Verifier code changed");
        require(verifier.verifyingKeyHash() == verifierKeyHash, "Verifier key changed");
        require(
            verifier.semanticProofProfileHash() == semanticProofProfileHash,
            "Semantic proof profile changed"
        );
        require(
            verifier.soraFinalityAnchorHash() == soraFinalityAnchorHash,
            "SORA finality anchor changed"
        );
        uint32 sourceDomain;
        bytes32 commitmentRoot;
        (messageId, sourceDomain, commitmentRoot) = verifier.verifySccpMessageProof(
            proofBytes, publicInputs, statementHash, destinationBindingHash, routeConfigHash
        );
        require(messageId == expectedMessageId, "Verifier message mismatch");
        require(sourceDomain == DOMAIN_SORA, "Unexpected source domain");
        require(commitmentRoot == publicInputs[3] && commitmentRoot != bytes32(0),
            "Commitment root mismatch");
    }

    function _parseTairaToTronTransfer(bytes memory payload)
        private view returns (address recipient, uint256 amount)
    {
        uint256 offset = 0;
        require(_readU8(payload, offset++) == 2, "Not a Transfer payload");
        require(_readU8(payload, offset++) == 1, "Unsupported payload version");
        require(_readU32Le(payload, offset) == DOMAIN_SORA, "Wrong source domain"); offset += 4;
        require(_readU32Le(payload, offset) == DOMAIN_TRON, "Wrong destination domain"); offset += 4;
        offset += 8;
        require(_readU32Le(payload, offset) == routeRevision, "Wrong route revision"); offset += 4;
        require(_readU32Le(payload, offset) == DOMAIN_SORA, "Wrong asset home"); offset += 4;
        require(_readU8(payload, offset++) == CODEC_TEXT, "Wrong asset codec");
        uint256 start; uint256 length;
        (start, length, offset) = _readVec(payload, offset);
        require(_equals(payload, start, length, ASSET_ID), "Wrong asset");
        amount = _readU128Le(payload, offset); offset += 16;
        require(amount != 0, "Zero amount");
        require(_readU8(payload, offset++) == CODEC_TEXT, "Wrong sender codec");
        (start, length, offset) = _readVec(payload, offset);
        require(_canonicalTextRange(payload, start, length), "Noncanonical sender");
        require(_readU8(payload, offset++) == CODEC_TRON21, "Wrong recipient codec");
        (start, length, offset) = _readVec(payload, offset);
        require(length == 21 && payload[start] == bytes1(0x41), "Wrong TRON recipient");
        uint160 raw;
        for (uint256 i = 1; i < 21; i++) raw = (raw << 8) | uint160(uint8(payload[start + i]));
        require(raw != 0, "Zero recipient"); recipient = address(raw);
        require(_readU8(payload, offset++) == CODEC_TEXT, "Wrong route codec");
        (start, length, offset) = _readVec(payload, offset);
        require(_equals(payload, start, length, ROUTE_ID), "Wrong route");
        require(offset == payload.length, "Trailing payload bytes");
    }

    function _networkIdWord(uint8 profile) private pure returns (bytes32) {
        if (profile == 10) return bytes32(uint256(0x2b6653dc));
        if (profile == 11) return bytes32(uint256(0xcd8690dc));
        if (profile == 12) return bytes32(uint256(0x94a9059e));
        revert("Unsupported TRON profile");
    }
    function _tronAddressWord(address account) private pure returns (bytes32) {
        return bytes32((uint256(0x41) << 160) | uint256(uint160(account)));
    }
    function _readU8(bytes memory value, uint256 offset) private pure returns (uint8) {
        require(offset < value.length, "Truncated payload"); return uint8(value[offset]);
    }
    function _readU32Le(bytes memory value, uint256 offset) private pure returns (uint32 out) {
        require(offset <= value.length && value.length - offset >= 4, "Truncated payload");
        for (uint256 i = 0; i < 4; i++) {
            out |= uint32(uint8(value[offset + i])) << uint32(i * 8);
        }
    }
    function _readU128Le(bytes memory value, uint256 offset) private pure returns (uint256 out) {
        require(offset <= value.length && value.length - offset >= 16, "Truncated payload");
        for (uint256 i = 0; i < 16; i++) out |= uint256(uint8(value[offset + i])) << (i * 8);
    }
    function _readVec(bytes memory value, uint256 offset)
        private pure returns (uint256 start, uint256 length, uint256 next)
    {
        length = _readU32Le(value, offset); start = offset + 4;
        require(start <= value.length && value.length - start >= length, "Truncated vector");
        next = start + length;
    }
    function _equals(bytes memory value, uint256 start, uint256 length, bytes memory expected)
        private pure returns (bool)
    {
        if (length != expected.length) return false;
        for (uint256 i = 0; i < length; i++) if (value[start + i] != expected[i]) return false;
        return true;
    }
    function _canonicalTextRange(bytes memory value, uint256 start, uint256 length)
        private pure returns (bool)
    {
        if (length == 0 || length > 256) return false;
        for (uint256 i = 0; i < length; i++) {
            uint8 c = uint8(value[start + i]); if (c < 0x21 || c > 0x7e) return false;
        }
        return true;
    }
    function _codeHash(address account) private view returns (bytes32 codeHash) {
        assembly { codeHash := extcodehash(account) }
    }
}

// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

import "./ISccpMessageVerifier.sol";
import "./SccpExactTransferCodec.sol";

interface ITairaXorExactEvmToken {
    function bridge() external view returns (address);
    function mint(address to, uint256 value) external returns (bool);
    function burnFrom(address from, uint256 value) external returns (bool);
}

/**
 * @title TairaXorExactEvmSccpBridge
 * @dev Shared implementation for the concrete Ethereum and BSC XOR routes.
 * The constructor accepts only a closed SCCP network profile and verifies the
 * executing chain id. There is no owner, arbitrary digest submission, mutable
 * route, configurable asset, signer committee, or proof bypass.
 */
abstract contract TairaXorExactEvmSccpBridge {
    uint32 internal constant DOMAIN_SORA = 0;
    uint32 internal constant DOMAIN_ETHEREUM = 1;
    uint32 internal constant DOMAIN_BSC = 2;
    uint8 internal constant CODEC_TEXT = 1;
    uint8 internal constant CODEC_EVM20 = 2;
    uint256 internal constant TAIRA_TO_TOKEN_SCALE = 1000000000;
    uint256 private constant MAX_U128 = (uint256(1) << 128) - 1;
    bytes private constant ASSET_ID = "xor";
    bytes private constant ETHEREUM_ROUTE_ID = "taira_eth_xor";
    bytes private constant BSC_ROUTE_ID = "taira_bsc_xor";
    bytes32 private constant DESTINATION_BINDING_SEPARATOR =
        keccak256("iroha:sccp:evm-destination-binding:v1");
    bytes32 private constant ROUTE_CONFIG_SEPARATOR =
        keccak256("sccp:concrete-route-config:v1");
    bytes32 private constant VERIFIER_BACKEND_HASH = keccak256("evm-groth16-bn254-v1");
    bytes32 private constant PROOF_FAMILY_HASH = keccak256("stark-fri-v1");
    bytes32 private constant EMPTY_CODE_HASH = keccak256("");

    ITairaXorExactEvmToken public immutable token;
    ISccpMessageVerifier public immutable verifier;
    uint32 public immutable externalDomain;
    uint8 public immutable networkProfile;
    uint32 public immutable routeRevision;
    uint256 public immutable externalChainId;
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
        uint256 tokenAmount,
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
        uint32 configuredExternalDomain,
        uint8 configuredNetworkProfile,
        uint32 configuredRouteRevision
    ) {
        require(tokenAddress != address(0) && verifierAddress != address(0), "Zero bridge address");
        require(tokenAddress != verifierAddress, "Bridge roles must differ");
        require(_profileDomain(configuredNetworkProfile) == configuredExternalDomain,
            "Profile/domain mismatch");
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
        uint256 expectedChainId = _profileChainId(configuredNetworkProfile);
        require(_chainId() == expectedChainId, "Wrong EVM chain id");

        ITairaXorExactEvmToken configuredToken = ITairaXorExactEvmToken(tokenAddress);
        require(configuredToken.bridge() == address(this), "Token route mismatch");
        bytes32 actualTokenCodeHash = _codeHash(tokenAddress);
        require(actualTokenCodeHash != bytes32(0) && actualTokenCodeHash != EMPTY_CODE_HASH,
            "Token contract is required");
        bytes32 actualVerifierCodeHash = _validatedVerifierCodeHash(
            verifierAddress,
            expectedVerifierCodeHash,
            expectedVerifierKeyHash,
            expectedSemanticProofProfileHash,
            expectedSoraFinalityAnchorHash
        );
        destinationBindingHash = _destinationBinding(
            expectedChainId,
            configuredExternalDomain,
            verifierAddress,
            actualVerifierCodeHash,
            expectedVerifierKeyHash,
            expectedSemanticProofProfileHash,
            expectedSoraFinalityAnchorHash
        );
        bytes32 inboundLaneHash;
        bytes32 outboundLaneHash;
        {
            bytes memory externalNetwork = _network(configuredNetworkProfile);
            bytes memory taira = SccpExactTransferCodec.tairaNetwork();
            inboundLaneHash = SccpExactTransferCodec.laneHashEvm(
                SccpExactTransferCodec.lane(externalNetwork, taira)
            );
            outboundLaneHash = SccpExactTransferCodec.laneHashEvm(
                SccpExactTransferCodec.lane(taira, externalNetwork)
            );
        }

        token = configuredToken;
        verifier = ISccpMessageVerifier(verifierAddress);
        externalDomain = configuredExternalDomain;
        networkProfile = configuredNetworkProfile;
        routeRevision = configuredRouteRevision;
        externalChainId = expectedChainId;
        tokenCodeHash = actualTokenCodeHash;
        verifierCodeHash = actualVerifierCodeHash;
        verifierKeyHash = expectedVerifierKeyHash;
        semanticProofProfileHash = expectedSemanticProofProfileHash;
        soraFinalityAnchorHash = expectedSoraFinalityAnchorHash;
        sourceLaneHash = inboundLaneHash;
        destinationLaneHash = outboundLaneHash;
        bytes32 deploymentConfigHash = keccak256(abi.encode(
            tokenAddress,
            actualTokenCodeHash,
            verifierAddress,
            actualVerifierCodeHash,
            expectedVerifierKeyHash,
            expectedSemanticProofProfileHash,
            expectedSoraFinalityAnchorHash
        ));
        bytes32 assetRouteConfigHash = keccak256(abi.encode(
            keccak256(ASSET_ID),
            keccak256(_routeIdForDomain(configuredExternalDomain)),
            configuredRouteRevision,
            TAIRA_TO_TOKEN_SCALE
        ));
        routeConfigHash = keccak256(abi.encode(
            ROUTE_CONFIG_SEPARATOR,
            configuredExternalDomain,
            configuredNetworkProfile,
            expectedChainId,
            inboundLaneHash,
            outboundLaneHash,
            deploymentConfigHash,
            assetRouteConfigHash
        ));
    }

    /** Burn wrapped XOR and emit one exact external-EVM-to-Taira statement. */
    function transferToTaira(bytes calldata tairaRecipient, uint256 tokenAmount)
        external
        nonReentrant
        returns (bytes32 messageId)
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
        SccpExactTransferCodec.TransferFields memory fields;
        fields.sourceDomain = externalDomain;
        fields.destinationDomain = DOMAIN_SORA;
        fields.nonce = nonce;
        fields.routeRevision = routeRevision;
        fields.assetHomeDomain = DOMAIN_SORA;
        fields.assetId = ASSET_ID;
        fields.amount = tairaAmount;
        fields.senderCodec = CODEC_EVM20;
        fields.sender = abi.encodePacked(msg.sender);
        fields.recipientCodec = CODEC_TEXT;
        fields.recipient = recipient;
        fields.routeId = _routeId();
        bytes memory payload = SccpExactTransferCodec.transferPayload(fields);
        bytes memory exactLane = SccpExactTransferCodec.lane(
            _network(networkProfile), SccpExactTransferCodec.tairaNetwork()
        );
        messageId = SccpExactTransferCodec.messageId(exactLane, payload);
        bytes32 canonicalPayloadHash = SccpExactTransferCodec.payloadHashEvm(payload);
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

    /** Verify one exact Taira-to-external-EVM proof and mint wrapped XOR. */
    function finalizeFromTaira(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes calldata canonicalPayloadBytes
    ) external nonReentrant returns (bytes32 messageId) {
        require(statementHash != bytes32(0), "Statement hash is required");
        require(publicInputs[2] == bytes32(uint256(externalDomain)), "Unexpected target domain");
        require(_codeHash(address(token)) == tokenCodeHash, "Token code changed");
        bytes memory payload = canonicalPayloadBytes;
        (address recipient, uint256 tairaAmount) = _parseTairaToEvmTransfer(payload);
        bytes32 canonicalPayloadHash = SccpExactTransferCodec.payloadHashEvm(payload);
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

    function sourceEventDigest(bytes32 messageId, bytes32 canonicalPayloadHash)
        external view returns (bytes32)
    {
        return SccpExactTransferCodec.sourceEventDigest(
            sourceLaneHash, messageId, canonicalPayloadHash
        );
    }

    function sccpPayloadHash(bytes calldata canonicalPayload) external view returns (bytes32) {
        bytes memory payload = canonicalPayload;
        return SccpExactTransferCodec.payloadHashEvm(payload);
    }

    function sccpDestinationMessageId(bytes calldata canonicalPayload)
        external view returns (bytes32)
    {
        bytes memory payload = canonicalPayload;
        return _destinationMessageId(payload);
    }

    function _destinationBinding(
        uint256 chainId,
        uint32 counterpartyDomain,
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
            bytes32(chainId),
            uint256(DOMAIN_SORA),
            uint256(counterpartyDomain),
            verifierAddress,
            address(this),
            codeHash,
            keyHash,
            semanticProfileHash,
            finalityAnchorHash
        ));
    }

    function _destinationMessageId(bytes memory payload) private view returns (bytes32) {
        return SccpExactTransferCodec.messageId(
            SccpExactTransferCodec.lane(
                SccpExactTransferCodec.tairaNetwork(), _network(networkProfile)
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
        require(_verifyingKeyHash(address(verifier)) == verifierKeyHash, "Verifier key changed");
        require(
            _semanticProofProfileHash(address(verifier)) == semanticProofProfileHash,
            "Semantic proof profile changed"
        );
        require(
            _soraFinalityAnchorHash(address(verifier)) == soraFinalityAnchorHash,
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

    function _parseTairaToEvmTransfer(bytes memory payload)
        private view returns (address recipient, uint256 amount)
    {
        uint256 offset = 0;
        require(_readU8(payload, offset++) == 2, "Not a Transfer payload");
        require(_readU8(payload, offset++) == 1, "Unsupported payload version");
        require(_readU32Le(payload, offset) == DOMAIN_SORA, "Wrong source domain"); offset += 4;
        require(_readU32Le(payload, offset) == externalDomain, "Wrong destination domain"); offset += 4;
        offset += 8; // nonce is committed by the exact lane message id
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
        require(_readU8(payload, offset++) == CODEC_EVM20, "Wrong recipient codec");
        (start, length, offset) = _readVec(payload, offset);
        require(length == 20, "Wrong recipient length");
        uint160 raw;
        for (uint256 i = 0; i < 20; i++) {
            raw = (raw << 8) | uint160(uint8(payload[start + i]));
        }
        require(raw != 0, "Zero recipient");
        recipient = address(raw);
        require(_readU8(payload, offset++) == CODEC_TEXT, "Wrong route codec");
        (start, length, offset) = _readVec(payload, offset);
        require(_equals(payload, start, length, _routeId()), "Wrong route");
        require(offset == payload.length, "Trailing payload bytes");
    }

    function _routeId() private view returns (bytes memory) {
        return _routeIdForDomain(externalDomain);
    }

    function _routeIdForDomain(uint32 domain) private pure returns (bytes memory) {
        if (domain == DOMAIN_ETHEREUM) return ETHEREUM_ROUTE_ID;
        require(domain == DOMAIN_BSC, "Unsupported EVM domain");
        return BSC_ROUTE_ID;
    }

    function _network(uint8 profile) private pure returns (bytes memory) {
        if (profile == 2 || profile == 3) return SccpExactTransferCodec.ethereumNetwork(profile);
        if (profile == 4 || profile == 5) return SccpExactTransferCodec.bscNetwork(profile);
        revert("Unsupported EVM profile");
    }

    function _profileDomain(uint8 profile) private pure returns (uint32) {
        if (profile == 2 || profile == 3) return DOMAIN_ETHEREUM;
        if (profile == 4 || profile == 5) return DOMAIN_BSC;
        revert("Unsupported EVM profile");
    }

    function _profileChainId(uint8 profile) private pure returns (uint256) {
        if (profile == 2) return 1;
        if (profile == 3) return 11155111;
        if (profile == 4) return 56;
        if (profile == 5) return 97;
        revert("Unsupported EVM profile");
    }

    function _validatedVerifierCodeHash(
        address verifierAddress,
        bytes32 expectedCodeHash,
        bytes32 expectedKeyHash,
        bytes32 expectedSemanticProfileHash,
        bytes32 expectedFinalityAnchorHash
    ) private view returns (bytes32 actualCodeHash) {
        actualCodeHash = _codeHash(verifierAddress);
        require(expectedCodeHash != bytes32(0) && actualCodeHash == expectedCodeHash
            && actualCodeHash != EMPTY_CODE_HASH, "Verifier code hash mismatch");
        require(expectedKeyHash != bytes32(0)
            && _verifyingKeyHash(verifierAddress) == expectedKeyHash,
            "Verifier key hash mismatch");
        require(
            _semanticProofProfileHash(verifierAddress) == expectedSemanticProfileHash,
            "Semantic proof profile hash mismatch"
        );
        require(
            _soraFinalityAnchorHash(verifierAddress) == expectedFinalityAnchorHash,
            "SORA finality anchor hash mismatch"
        );
    }

    function _readU8(bytes memory value, uint256 offset) private pure returns (uint8) {
        require(offset < value.length, "Truncated payload");
        return uint8(value[offset]);
    }

    function _readU32Le(bytes memory value, uint256 offset) private pure returns (uint32 out) {
        require(offset <= value.length && value.length - offset >= 4, "Truncated payload");
        for (uint256 i = 0; i < 4; i++) {
            out |= uint32(uint8(value[offset + i])) << uint32(i * 8);
        }
    }

    function _readU128Le(bytes memory value, uint256 offset) private pure returns (uint256 out) {
        require(offset <= value.length && value.length - offset >= 16, "Truncated payload");
        for (uint256 i = 0; i < 16; i++) {
            out |= uint256(uint8(value[offset + i])) << (i * 8);
        }
    }

    function _readVec(bytes memory value, uint256 offset)
        private pure returns (uint256 start, uint256 length, uint256 next)
    {
        length = _readU32Le(value, offset);
        start = offset + 4;
        require(start <= value.length && value.length - start >= length, "Truncated vector");
        next = start + length;
    }

    function _equals(bytes memory value, uint256 start, uint256 length, bytes memory expected)
        private pure returns (bool)
    {
        if (length != expected.length) return false;
        for (uint256 i = 0; i < length; i++) {
            if (value[start + i] != expected[i]) return false;
        }
        return true;
    }

    function _canonicalTextRange(bytes memory value, uint256 start, uint256 length)
        private pure returns (bool)
    {
        if (length == 0 || length > 256) return false;
        for (uint256 i = 0; i < length; i++) {
            uint8 character = uint8(value[start + i]);
            if (character < 0x21 || character > 0x7e) return false;
        }
        return true;
    }

    function _chainId() private pure returns (uint256 value) {
        assembly { value := chainid() }
    }

    function _codeHash(address account) private view returns (bytes32 codeHash) {
        assembly { codeHash := extcodehash(account) }
    }

    function _verifyingKeyHash(address account) private view returns (bytes32 keyHash) {
        (bool success, bytes memory data) = account.staticcall(
            abi.encodeWithSignature("verifyingKeyHash()")
        );
        require(success && data.length == 32, "Verifier key unavailable");
        keyHash = abi.decode(data, (bytes32));
    }

    function _semanticProofProfileHash(address account) private view returns (bytes32 profileHash) {
        (bool success, bytes memory data) = account.staticcall(
            abi.encodeWithSignature("semanticProofProfileHash()")
        );
        require(success && data.length == 32, "Semantic proof profile unavailable");
        profileHash = abi.decode(data, (bytes32));
    }

    function _soraFinalityAnchorHash(address account) private view returns (bytes32 anchorHash) {
        (bool success, bytes memory data) = account.staticcall(
            abi.encodeWithSignature("soraFinalityAnchorHash()")
        );
        require(success && data.length == 32, "SORA finality anchor unavailable");
        anchorHash = abi.decode(data, (bytes32));
    }
}
